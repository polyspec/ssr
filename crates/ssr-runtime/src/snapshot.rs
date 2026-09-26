use crate::{Error, web};
use deno_core::{JsRuntime, v8};
use sha2::{Digest, Sha256};
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::thread;
use std::time::Duration;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct Key {
    bundle_hash: [u8; 32],
    deno_core_version: &'static str,
    library_version: &'static str,
}

impl Key {
    fn new(bundle: &[u8]) -> Self {
        Self {
            bundle_hash: Sha256::digest(bundle).into(),
            deno_core_version: env!("SSR_DENO_CORE_VERSION"),
            library_version: env!("CARGO_PKG_VERSION"),
        }
    }
}

pub(crate) struct Snapshot {
    bytes: Vec<u8>,
}

impl Snapshot {
    pub(crate) fn isolate(&self) -> v8::OwnedIsolate {
        let parameters = v8::CreateParams::default()
            .external_references(Cow::Owned(web::external_references()))
            .snapshot_blob(v8::StartupData::from(self.bytes.clone()));
        v8::Isolate::new(parameters)
    }
}

static CACHE: OnceLock<Mutex<HashMap<Key, Weak<Snapshot>>>> = OnceLock::new();

pub(crate) fn get(bundle: &str, timeout: Duration) -> Result<Arc<Snapshot>, Error> {
    get_with_key(bundle, Key::new(bundle.as_bytes()), timeout)
}

fn get_with_key(bundle: &str, key: Key, timeout: Duration) -> Result<Arc<Snapshot>, Error> {
    let mut cache = CACHE
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|_| Error::Snapshot("snapshot cache lock failed"))?;
    if let Some(snapshot) = cache.get(&key).and_then(Weak::upgrade) {
        return Ok(snapshot);
    }
    let snapshot = Arc::new(Snapshot {
        bytes: create(bundle, timeout)?,
    });
    cache.retain(|_, entry| entry.strong_count() > 0);
    cache.insert(key, Arc::downgrade(&snapshot));
    Ok(snapshot)
}

fn create(bundle: &str, timeout: Duration) -> Result<Vec<u8>, Error> {
    JsRuntime::init_platform(None);
    let mut creator =
        v8::Isolate::snapshot_creator(Some(Cow::Owned(web::external_references())), None);
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
    let initialization = (|| -> Result<(), Error> {
        v8::scope!(let scope, &mut creator);
        let context = v8::Local::new(scope, &context);
        let scope = &mut v8::ContextScope::new(scope, context);
        v8::tc_scope!(let scope, scope);
        web::install(scope, context)?;
        let source = v8::String::new(scope, bundle)
            .ok_or(Error::InvalidBundle("bundle exceeds V8 string limit"))?;
        let name = v8::String::new(scope, "server.js")
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
        let script = v8::Script::compile(scope, source, Some(&origin))
            .ok_or(Error::InvalidBundle("bundle compilation failed"))?;
        script
            .run(scope)
            .ok_or(Error::InvalidBundle("bundle initialization failed"))?;
        let render_name = v8::String::new(scope, "render")
            .ok_or(Error::InvalidBundle("render name unavailable"))?;
        let render = context
            .global(scope)
            .get(scope, render_name.into())
            .ok_or(Error::InvalidBundle("render lookup failed"))?;
        if !render.is_function() {
            return Err(Error::InvalidBundle("global render function required"));
        }
        Ok(())
    })();
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
    initialization?;
    let blob = blob?;
    Ok(blob.to_vec())
}

#[cfg(test)]
mod tests {
    use super::{Key, get_with_key};
    use std::sync::Arc;
    use std::time::Duration;

    #[test]
    fn key_changes_for_bundle_and_versions() {
        let original = Key::new(b"one");
        assert_eq!(original, Key::new(b"one"));
        assert_ne!(original, Key::new(b"two"));
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
        let bundle = "function render(props, state) { return {head:'', html:'ok', state}; }";
        let key = Key::new(bundle.as_bytes());
        let first = get_with_key(bundle, key.clone(), Duration::from_secs(2)).unwrap();
        let reused = get_with_key(bundle, key.clone(), Duration::from_secs(2)).unwrap();
        assert!(Arc::ptr_eq(&first, &reused));
        let changed_bundle = get_with_key(
            "function render(props, state) { return {head:'', html:'changed', state}; }",
            Key::new(b"function render(props, state) { return {head:'', html:'changed', state}; }"),
            Duration::from_secs(2),
        )
        .unwrap();
        assert!(!Arc::ptr_eq(&first, &changed_bundle));
        let changed_core = get_with_key(
            bundle,
            Key {
                deno_core_version: "changed",
                ..key.clone()
            },
            Duration::from_secs(2),
        )
        .unwrap();
        assert!(!Arc::ptr_eq(&first, &changed_core));
        let changed_library = get_with_key(
            bundle,
            Key {
                library_version: "changed",
                ..key
            },
            Duration::from_secs(2),
        )
        .unwrap();
        assert!(!Arc::ptr_eq(&first, &changed_library));
    }
}
