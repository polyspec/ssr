use crate::{Error, module, web};
use deno_core::{JsRuntime, v8};
use sha2::{Digest, Sha256};
use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, OnceLock, Weak};
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
    group: Arc<v8::IsolateGroup>,
    kind: SnapshotKind,
}

impl Snapshot {
    pub(crate) fn isolate(&self) -> v8::OwnedIsolate {
        let parameters = v8::CreateParams::default()
            .external_references(Cow::Owned(web::external_references()))
            .snapshot_blob(v8::StartupData::from(self.bytes.clone()));
        let mut isolate = self.group.new_isolate(parameters);
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

static CACHE: OnceLock<Mutex<HashMap<Key, Weak<Snapshot>>>> = OnceLock::new();

pub(crate) fn get(bundle: &module::Sources, timeout: Duration) -> Result<Arc<Snapshot>, Error> {
    get_with_key(bundle, Key::new(bundle), timeout)
}

fn get_with_key(
    bundle: &module::Sources,
    key: Key,
    timeout: Duration,
) -> Result<Arc<Snapshot>, Error> {
    get_or_create(key, |group| {
        let sources = Arc::new(bundle.clone());
        let (bytes, module_indices) = create_modules(group, Arc::clone(&sources), timeout)?;
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
    get_or_create(key, |group| {
        let bytes = create_react(
            group,
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
    create: impl FnOnce(&v8::IsolateGroup) -> Result<(Vec<u8>, SnapshotKind), Error>,
) -> Result<Arc<Snapshot>, Error> {
    let mut cache = CACHE
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|_| Error::Snapshot("snapshot cache lock failed"))?;
    if let Some(snapshot) = cache.get(&key).and_then(Weak::upgrade) {
        return Ok(snapshot);
    }
    JsRuntime::init_platform(None);
    let group = Arc::new(
        v8::IsolateGroup::create()
            .ok_or(Error::Snapshot("independent isolate group unavailable"))?,
    );
    let (bytes, kind) = create(&group)?;
    let snapshot = Arc::new(Snapshot { bytes, group, kind });
    cache.retain(|_, entry| entry.strong_count() > 0);
    cache.insert(key, Arc::downgrade(&snapshot));
    Ok(snapshot)
}

fn create_modules(
    group: &v8::IsolateGroup,
    bundle: Arc<module::Sources>,
    timeout: Duration,
) -> Result<(Vec<u8>, BTreeMap<String, usize>), Error> {
    create_with(group, timeout, true, |creator, default_context| {
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
    group: &v8::IsolateGroup,
    framework_path: &str,
    framework: &str,
    application_path: &str,
    application: &str,
    timeout: Duration,
) -> Result<Vec<u8>, Error> {
    let (bytes, ()) = create_with(group, timeout, false, |creator, default_context| {
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
    group: &v8::IsolateGroup,
    timeout: Duration,
    modules: bool,
    initialize: impl FnOnce(&mut v8::OwnedIsolate, &v8::Global<v8::Context>) -> Result<T, Error>,
) -> Result<(Vec<u8>, T), Error> {
    let mut creator = v8::Isolate::snapshot_creator_in_group(
        group,
        Some(Cow::Owned(web::external_references())),
        None,
        None,
    );
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
    fn matching_key_reuses_snapshot_and_mismatch_creates_one() {
        let source_bundle =
            bundle("export function render(props, state) { return {head:'', html:'ok', state}; }");
        let key = Key::new(&source_bundle);
        let first = get_with_key(&source_bundle, key.clone(), Duration::from_secs(2)).unwrap();
        let reused = get_with_key(&source_bundle, key.clone(), Duration::from_secs(2)).unwrap();
        assert!(Arc::ptr_eq(&first, &reused));
        let changed = bundle(
            "export function render(props, state) { return {head:'', html:'changed', state}; }",
        );
        let changed_bundle =
            get_with_key(&changed, Key::new(&changed), Duration::from_secs(2)).unwrap();
        assert!(!Arc::ptr_eq(&first, &changed_bundle));
        let changed_core = get_with_key(
            &source_bundle,
            Key {
                deno_core_version: "changed",
                ..key.clone()
            },
            Duration::from_secs(2),
        )
        .unwrap();
        assert!(!Arc::ptr_eq(&first, &changed_core));
        let changed_library = get_with_key(
            &source_bundle,
            Key {
                library_version: "changed",
                ..key
            },
            Duration::from_secs(2),
        )
        .unwrap();
        assert!(!Arc::ptr_eq(&first, &changed_library));
    }

    #[test]
    fn different_application_snapshots_restore_in_parallel() {
        let values = ["Ada", "Bea", "Cia"];
        let snapshots: Vec<_> = values
            .iter()
            .map(|value| {
                get(
                    &bundle(&format!(
                        "globalThis.marker = '{value}'; export function render(props, state) {{ return {{head:'', html:props.name, state}}; }}"
                    )),
                    Duration::from_secs(10),
                )
                .unwrap()
            })
            .collect();
        for _ in 0..10 {
            let barrier = Arc::new(Barrier::new(values.len()));
            std::thread::scope(|threads| {
                let workers: Vec<_> = snapshots
                    .iter()
                    .zip(values)
                    .map(|(snapshot, expected)| {
                        let barrier = Arc::clone(&barrier);
                        threads.spawn(move || {
                            barrier.wait();
                            let mut isolate = snapshot.isolate();
                            v8::scope!(let scope, &mut isolate);
                            let context = v8::Context::new(scope, Default::default());
                            let scope = &mut v8::ContextScope::new(scope, context);
                            let source = v8::String::new(scope, "globalThis.marker").unwrap();
                            let script = v8::Script::compile(scope, source, None).unwrap();
                            let actual = script.run(scope).unwrap().to_rust_string_lossy(scope);
                            assert_eq!(actual, expected);
                        })
                    })
                    .collect();
                for worker in workers {
                    worker.join().unwrap();
                }
            });
        }
    }

    #[test]
    fn react_application_snapshots_restore_in_parallel() {
        const FRAMEWORK: &str = "globalThis.__ssrReact = {}; globalThis.__ssrJsxRuntime = {}; globalThis.render = () => null";
        let snapshots = ["Ada", "Bea", "Cia"].map(|name| {
            let application = format!("globalThis.__ssrApp = () => '{name}'");
            (
                get_react(
                    "server/framework.js",
                    FRAMEWORK,
                    "server/application.js",
                    &application,
                    Duration::from_secs(3),
                )
                .unwrap(),
                name,
            )
        });
        for _ in 0..10 {
            let barrier = Barrier::new(4);
            std::thread::scope(|threads| {
                for (snapshot, expected) in &snapshots {
                    let barrier = &barrier;
                    threads.spawn(move || {
                        barrier.wait();
                        let mut isolate = snapshot.isolate();
                        v8::scope!(let scope, &mut isolate);
                        let application = v8::Context::new(scope, Default::default());
                        let scope = &mut v8::ContextScope::new(scope, application);
                        let source = v8::String::new(scope, "__ssrApp()").unwrap();
                        let actual = v8::Script::compile(scope, source, None)
                            .unwrap()
                            .run(scope)
                            .unwrap()
                            .to_rust_string_lossy(scope);
                        assert_eq!(actual, *expected);
                    });
                }
                barrier.wait();
            });
        }
    }
}
