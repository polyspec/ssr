use crate::{Error, module, web};
use deno_core::{JsRuntime, v8};
use sha2::{Digest, Sha256};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;

macro_rules! script_error {
    ($scope:expr) => {{
        if $scope.is_execution_terminating() {
            Error::Timeout
        } else {
            let message = $scope
                .exception()
                .and_then(|value| value.to_string($scope))
                .map(|value| value.to_rust_string_lossy($scope))
                .unwrap_or_else(|| "JavaScript failed without an exception".to_owned());
            let stack = $scope
                .stack_trace()
                .and_then(|value| value.to_string($scope))
                .map(|value| value.to_rust_string_lossy($scope));
            Error::JavaScript { message, stack }
        }
    }};
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct Key {
    bundle_hash: [u8; 32],
    deno_core_version: &'static str,
    library_version: &'static str,
}

impl Key {
    fn new(bundle: &module::Sources) -> Self {
        Self {
            bundle_hash: Sha256::digest(bundle.cache_bytes()).into(),
            deno_core_version: env!("SSR_DENO_CORE_VERSION"),
            library_version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn react(
        framework_path: &str,
        framework: &[u8],
        application_path: &str,
        application: &[u8],
    ) -> Self {
        let mut digest = Sha256::new();
        digest.update(b"react-framework");
        digest.update((framework_path.len() as u64).to_be_bytes());
        digest.update(framework_path.as_bytes());
        digest.update((framework.len() as u64).to_be_bytes());
        digest.update(framework);
        digest.update((application_path.len() as u64).to_be_bytes());
        digest.update(application_path.as_bytes());
        digest.update((application.len() as u64).to_be_bytes());
        digest.update(application);
        Self {
            bundle_hash: digest.finalize().into(),
            deno_core_version: env!("SSR_DENO_CORE_VERSION"),
            library_version: env!("CARGO_PKG_VERSION"),
        }
    }
}

enum SnapshotKind {
    Modules {
        sources: Arc<module::Sources>,
        module_indices: BTreeMap<String, usize>,
    },
    React,
}

pub(crate) struct Snapshot {
    bytes: Vec<u8>,
    kind: SnapshotKind,
}

impl Snapshot {
    pub(crate) fn isolate(&self, max_heap_bytes: usize) -> v8::OwnedIsolate {
        let parameters = v8::CreateParams::default()
            .heap_limits(0, max_heap_bytes)
            .external_references(Cow::Owned(web::external_references()))
            .snapshot_blob(v8::StartupData::from(self.bytes.clone()));
        let mut isolate = v8::Isolate::new(parameters);
        if matches!(&self.kind, SnapshotKind::Modules { .. }) {
            module::install_dynamic_import_callback(&mut isolate);
        }
        isolate
    }

    pub(crate) fn module_data(&self) -> Option<(Arc<module::Sources>, BTreeMap<String, usize>)> {
        match &self.kind {
            SnapshotKind::Modules {
                sources,
                module_indices,
            } => Some((Arc::clone(sources), module_indices.clone())),
            SnapshotKind::React => None,
        }
    }

    pub(crate) fn is_react(&self) -> bool {
        matches!(&self.kind, SnapshotKind::React)
    }
}

struct Selection {
    key: Key,
    snapshot: Arc<Snapshot>,
}

static CACHE: OnceLock<Mutex<Option<Selection>>> = OnceLock::new();

pub(crate) fn get(bundle: &module::Sources, timeout: Duration) -> Result<Arc<Snapshot>, Error> {
    get_with_key(bundle, Key::new(bundle), timeout)
}

fn get_with_key(
    bundle: &module::Sources,
    key: Key,
    timeout: Duration,
) -> Result<Arc<Snapshot>, Error> {
    get_or_create(key, || {
        let sources = Arc::new(bundle.clone());
        let (bytes, module_indices) = create_modules(Arc::clone(&sources), timeout)?;
        Ok((
            bytes,
            SnapshotKind::Modules {
                sources,
                module_indices,
            },
        ))
    })
}

pub(crate) fn get_react(
    framework_path: &str,
    framework: &str,
    application_path: &str,
    application: &str,
    timeout: Duration,
) -> Result<Arc<Snapshot>, Error> {
    let key = Key::react(
        framework_path,
        framework.as_bytes(),
        application_path,
        application.as_bytes(),
    );
    get_or_create(key, || {
        let bytes = create_react(
            framework_path,
            framework,
            application_path,
            application,
            timeout,
        )?;
        Ok((bytes, SnapshotKind::React))
    })
}

fn get_or_create(
    key: Key,
    create: impl FnOnce() -> Result<(Vec<u8>, SnapshotKind), Error>,
) -> Result<Arc<Snapshot>, Error> {
    let mut cache = CACHE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .map_err(|_| Error::Snapshot("snapshot cache lock failed"))?;
    if let Some(selected) = &*cache {
        if selected.key != key {
            return Err(Error::Snapshot(
                "renderer process already selected another snapshot key",
            ));
        }
        return Ok(Arc::clone(&selected.snapshot));
    }
    JsRuntime::init_platform(None);
    let (bytes, kind) = create()?;
    let snapshot = Arc::new(Snapshot { bytes, kind });
    *cache = Some(Selection {
        key,
        snapshot: Arc::clone(&snapshot),
    });
    Ok(snapshot)
}

fn create_modules(
    bundle: Arc<module::Sources>,
    timeout: Duration,
) -> Result<(Vec<u8>, BTreeMap<String, usize>), Error> {
    create_with(timeout, true, |creator, default_context| {
        v8::scope!(let scope, creator);
        let context = v8::Local::new(scope, default_context);
        let scope = &mut v8::ContextScope::new(scope, context);
        v8::tc_scope!(let scope, scope);
        web::install(scope, context)?;
        let result = module::initialize(scope, context, Arc::clone(&bundle));
        context.clear_all_slots();
        if result.is_err() && scope.has_caught() {
            if scope.has_terminated() {
                return Err(Error::Timeout);
            }
            let message = scope
                .exception()
                .and_then(|value| value.to_string(scope))
                .map(|value| value.to_rust_string_lossy(scope))
                .ok_or(Error::InvalidBundle(
                    "server module failed without an error message",
                ))?;
            let stack = scope
                .stack_trace()
                .and_then(|value| value.to_string(scope))
                .map(|value| value.to_rust_string_lossy(scope));
            return Err(Error::JavaScript { message, stack });
        }
        result
    })
}

fn create_react(
    framework_path: &str,
    framework: &str,
    application_path: &str,
    application: &str,
    timeout: Duration,
) -> Result<Vec<u8>, Error> {
    let (bytes, ()) = create_with(timeout, false, |creator, default_context| {
        v8::scope!(let scope, creator);
        let context = v8::Local::new(scope, default_context);
        let scope = &mut v8::ContextScope::new(scope, context);
        web::install_framework(scope, context)?;
        run_react_framework(scope, context, framework, framework_path)?;
        let key = v8::String::new(scope, "setTimeout")
            .ok_or(Error::InvalidBundle("React scheduling name unavailable"))?;
        if context.global(scope).delete(scope, key.into()) != Some(true) {
            return Err(Error::InvalidBundle("React scheduling removal failed"));
        }
        run_bundle(scope, context, application, application_path, "__ssrApp")?;
        Ok(())
    })?;
    Ok(bytes)
}

fn run_react_framework(
    scope: &mut v8::PinScope<'_, '_>,
    context: v8::Local<v8::Context>,
    bundle: &str,
    script_name: &str,
) -> Result<(), Error> {
    v8::tc_scope!(let scope, scope);
    let source = v8::String::new(scope, bundle).ok_or(Error::InvalidBundle(
        "framework bundle exceeds V8 string limit",
    ))?;
    let name = v8::String::new(scope, script_name)
        .ok_or(Error::InvalidBundle("framework script name unavailable"))?;
    let origin = v8::ScriptOrigin::new(
        scope,
        name.into(),
        0,
        0,
        false,
        0,
        None,
        false,
        false,
        false,
        None,
    );
    let mut source = v8::script_compiler::Source::new(source, Some(&origin));
    let timer_name = v8::String::new(scope, "setTimeout")
        .ok_or(Error::InvalidBundle("React scheduling name unavailable"))?;
    let stream_name = v8::String::new(scope, "ReadableStream")
        .ok_or(Error::InvalidBundle("React stream name unavailable"))?;
    let timer = context
        .global(scope)
        .get(scope, timer_name.into())
        .ok_or(Error::InvalidBundle("React scheduling callback missing"))?;
    let function = v8::script_compiler::compile_function(
        scope,
        &mut source,
        &[timer_name, stream_name],
        &[],
        v8::script_compiler::CompileOptions::NoCompileOptions,
        v8::script_compiler::NoCacheReason::NoReason,
    )
    .ok_or_else(|| script_error!(scope))?;
    let undefined = v8::undefined(scope).into();
    function
        .call(scope, context.global(scope).into(), &[timer, undefined])
        .ok_or_else(|| script_error!(scope))?;
    for required in ["render", "__ssrReact", "__ssrJsxRuntime"] {
        let key = v8::String::new(scope, required).ok_or(Error::InvalidBundle(
            "React framework export name unavailable",
        ))?;
        let value = context
            .global(scope)
            .get(scope, key.into())
            .ok_or(Error::InvalidBundle("React framework export missing"))?;
        if required == "render" && !value.is_function() {
            return Err(Error::InvalidBundle("React render function required"));
        }
        if required != "render" && !value.is_object() {
            return Err(Error::InvalidBundle("React framework object required"));
        }
    }
    Ok(())
}

fn run_bundle(
    scope: &mut v8::PinScope<'_, '_>,
    context: v8::Local<v8::Context>,
    bundle: &str,
    script_name: &str,
    expected_function: &str,
) -> Result<(), Error> {
    v8::tc_scope!(let scope, scope);
    let source = v8::String::new(scope, bundle)
        .ok_or(Error::InvalidBundle("bundle exceeds V8 string limit"))?;
    let name = v8::String::new(scope, script_name)
        .ok_or(Error::InvalidBundle("script name unavailable"))?;
    let origin = v8::ScriptOrigin::new(
        scope,
        name.into(),
        0,
        0,
        false,
        0,
        None,
        false,
        false,
        false,
        None,
    );
    let script =
        v8::Script::compile(scope, source, Some(&origin)).ok_or_else(|| script_error!(scope))?;
    script.run(scope).ok_or_else(|| script_error!(scope))?;
    let function_name = v8::String::new(scope, expected_function)
        .ok_or(Error::InvalidBundle("render name unavailable"))?;
    let function = context
        .global(scope)
        .get(scope, function_name.into())
        .ok_or(Error::InvalidBundle("render lookup failed"))?;
    if !function.is_function() {
        return Err(Error::InvalidBundle("global render function required"));
    }
    Ok(())
}

fn create_with<T>(
    timeout: Duration,
    modules: bool,
    initialize: impl FnOnce(&mut v8::OwnedIsolate, &v8::Global<v8::Context>) -> Result<T, Error>,
) -> Result<(Vec<u8>, T), Error> {
    ssr_core::process::render(|| create_snapshot(timeout, modules, initialize))
        .map_err(Error::Snapshot)?
}

fn create_snapshot<T>(
    timeout: Duration,
    modules: bool,
    initialize: impl FnOnce(&mut v8::OwnedIsolate, &v8::Global<v8::Context>) -> Result<T, Error>,
) -> Result<(Vec<u8>, T), Error> {
    let mut creator =
        v8::Isolate::snapshot_creator(Some(Cow::Owned(web::external_references())), None);
    if modules {
        module::install_dynamic_import_callback(&mut creator);
    }
    let context = {
        v8::scope!(let scope, &mut creator);
        let context = v8::Context::new(scope, Default::default());
        scope.set_default_context(context);
        v8::Global::new(scope, context)
    };
    let handle = creator.thread_safe_handle();
    let (done_tx, done_rx) = std::sync::mpsc::channel::<()>();
    let watchdog = match thread::Builder::new()
        .name("ssr-snapshot-timeout".to_owned())
        .spawn(move || match done_rx.recv_timeout(timeout) {
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                handle.terminate_execution();
                true
            }
            Ok(()) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => false,
        }) {
        Ok(watchdog) => watchdog,
        Err(error) => {
            drop(context);
            drop(creator.create_blob(v8::FunctionCodeHandling::Clear));
            return Err(Error::WorkerStartup(error));
        }
    };
    let initialization = initialize(&mut creator, &context);
    drop(done_tx);
    let timed_out = watchdog
        .join()
        .map_err(|_| Error::Snapshot("snapshot timeout worker failed"));
    creator.cancel_terminate_execution();
    drop(context);
    let blob = creator
        .create_blob(v8::FunctionCodeHandling::Keep)
        .ok_or(Error::Snapshot("snapshot serialization failed"));
    if timed_out? {
        return Err(Error::Timeout);
    }
    let result = initialization?;
    let blob = blob?;
    Ok((blob.to_vec(), result))
}

#[cfg(test)]
mod tests {
    use super::{Key, get, get_react, get_with_key};
    use crate::module::Sources;
    use deno_core::v8;
    use std::sync::{Arc, Barrier};
    use std::time::Duration;

    #[tokio::test]
    async fn snapshot_creation_rejects_concurrent_svelte_compiler() {
        use ssr_build::{BuildConfig, build};
        use std::path::Path;
        use std::process::Command;
        const TEST: &str = "snapshot::tests::snapshot_creation_rejects_concurrent_svelte_compiler";
        if std::env::var_os("SSR_CONCURRENT_COMPILER").is_some() {
            let root = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tools/build-probe/tests/fixtures")
                .canonicalize()
                .unwrap();
            let generated = root.join("node_modules/.ssr-runtime-concurrent-compiler");
            std::fs::create_dir_all(&generated).unwrap();
            let server = generated.join("server.js");
            let client = generated.join("client.js");
            let source =
                "import App from '../../SvelteApp.svelte'; export const render = () => App;";
            std::fs::write(&server, source).unwrap();
            std::fs::write(&client, source).unwrap();
            let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(1);
            let (finish_tx, finish_rx) = std::sync::mpsc::sync_channel(1);
            let creator = std::thread::spawn(move || {
                deno_core::JsRuntime::init_platform(None);
                super::create_with(Duration::from_secs(10), false, |_, _| {
                    ready_tx.send(()).unwrap();
                    finish_rx.recv_timeout(Duration::from_secs(10)).unwrap();
                    Ok(())
                })
            });
            ready_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            let result = build(&BuildConfig {
                root: root.clone(),
                server_entry: server,
                client_entry: client,
                react_framework_entry: None,
                css_entry: root.join("app.css"),
                asset_route: "/assets".into(),
                dependencies: None,
            })
            .await;
            finish_tx.send(()).unwrap();
            creator.join().unwrap().unwrap();
            let diagnostic = result
                .as_ref()
                .map(|_| "compiled")
                .map_err(ToString::to_string);
            assert!(
                matches!(&result, Err(ssr_build::Error::JavaScript(message)) if message.contains("Svelte compilation cannot run while renderer initialization is active")),
                "compiler result: {diagnostic:?}"
            );
            return;
        }
        // The child's output is captured and named in a failure; the nextest limit of this case
        // bounds the child, which runs in the case's process group.
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST, "--nocapture"])
            .env("SSR_CONCURRENT_COMPILER", "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "the compiler process did not return the required error: {}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn bundle(source: &str) -> Sources {
        Sources::new(
            ("server/entry.js".into(), source.as_bytes().to_vec()),
            vec![],
        )
        .unwrap()
    }

    #[test]
    fn key_changes_for_bundle_and_versions() {
        let original = Key::new(&bundle("export function render() {}"));
        assert_eq!(original, Key::new(&bundle("export function render() {}")));
        assert_ne!(
            original,
            Key::new(&bundle("export function render() { return 1; }"))
        );
        assert_ne!(
            original,
            Key {
                deno_core_version: "changed",
                ..original.clone()
            }
        );
        assert_ne!(
            original,
            Key {
                library_version: "changed",
                ..original.clone()
            }
        );
    }

    #[test]
    fn matching_key_reuses_snapshot_and_mismatch_is_rejected() {
        let source_bundle =
            bundle("export function render(props, state) { return {head:'', html:'ok', state}; }");
        let key = Key::new(&source_bundle);
        let first = get_with_key(&source_bundle, key.clone(), Duration::from_secs(2)).unwrap();
        let reused = get_with_key(&source_bundle, key.clone(), Duration::from_secs(2)).unwrap();
        assert!(Arc::ptr_eq(&first, &reused));
        for changed in [
            Key::new(&bundle("export function render() {}")),
            Key {
                deno_core_version: "changed",
                ..key.clone()
            },
            Key {
                library_version: "changed",
                ..key.clone()
            },
        ] {
            assert!(get_with_key(&source_bundle, changed, Duration::from_secs(2)).is_err());
        }
    }

    #[test]
    fn same_application_snapshot_restores_in_parallel() {
        let snapshot = get(
            &bundle("globalThis.marker='Ada'; export function render() {}"),
            Duration::from_secs(3),
        )
        .unwrap();
        for _ in 0..10 {
            let barrier = Barrier::new(4);
            std::thread::scope(|threads| {
                for _ in 0..4 {
                    let barrier = &barrier;
                    let snapshot = &snapshot;
                    threads.spawn(move || {
                        barrier.wait();
                        let mut isolate = snapshot.isolate(134_217_728);
                        v8::scope!(let scope, &mut isolate);
                        let context = v8::Context::new(scope, Default::default());
                        let scope = &mut v8::ContextScope::new(scope, context);
                        let source = v8::String::new(scope, "globalThis.marker").unwrap();
                        let value = v8::Script::compile(scope, source, None)
                            .unwrap()
                            .run(scope)
                            .unwrap();
                        assert_eq!(value.to_rust_string_lossy(scope), "Ada");
                    });
                }
            });
        }
    }

    #[test]
    fn same_react_snapshot_restores_in_parallel() {
        const FRAMEWORK: &str = "globalThis.__ssrReact = {}; globalThis.__ssrJsxRuntime = {}; globalThis.render = () => null";
        let snapshot = get_react(
            "server/framework.js",
            FRAMEWORK,
            "server/application.js",
            "globalThis.__ssrApp = () => 'Ada'",
            Duration::from_secs(3),
        )
        .unwrap();
        for _ in 0..10 {
            let barrier = Barrier::new(4);
            std::thread::scope(|threads| {
                for _ in 0..4 {
                    let barrier = &barrier;
                    let snapshot = &snapshot;
                    threads.spawn(move || {
                        barrier.wait();
                        let mut isolate = snapshot.isolate(134_217_728);
                        v8::scope!(let scope, &mut isolate);
                        let context = v8::Context::new(scope, Default::default());
                        let scope = &mut v8::ContextScope::new(scope, context);
                        let source = v8::String::new(scope, "__ssrApp()").unwrap();
                        let value = v8::Script::compile(scope, source, None)
                            .unwrap()
                            .run(scope)
                            .unwrap();
                        assert_eq!(value.to_rust_string_lossy(scope), "Ada");
                    });
                }
            });
        }
    }
}
