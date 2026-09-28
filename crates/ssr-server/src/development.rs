mod body;
mod config;
pub use config::ProcessOptions;
use config::validate_excluded;
mod process;
mod supervisor;

use http::{Request, Response};
use hyper::body::{Body as HttpBody, Bytes};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use ssr_core::{Page, Render};
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, RwLock};
use std::thread::JoinHandle;
use tokio::sync::{mpsc, watch};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DevelopmentError(String);

impl fmt::Display for DevelopmentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for DevelopmentError {}

type Current = Arc<RwLock<Result<Arc<process::Generation>, DevelopmentError>>>;
type Worker = JoinHandle<Result<(), DevelopmentError>>;
type Ready = tokio::sync::oneshot::Receiver<Result<(), DevelopmentError>>;
type Finished = watch::Receiver<Option<Result<(), DevelopmentError>>>;
type Startup = (Worker, Ready, Finished);
type WatchEvents = (
    mpsc::Receiver<notify::Result<notify::Event>>,
    watch::Receiver<Option<DevelopmentError>>,
);

pub struct Development {
    current: Current,
    watcher: Mutex<Option<RecommendedWatcher>>,
    stop: watch::Sender<bool>,
    worker: Mutex<Option<Worker>>,
    finished: Finished,
}

impl Development {
    pub async fn start(
        roots: Vec<PathBuf>,
        excluded: Vec<PathBuf>,
        build: Command,
        render: impl Fn(&Path) -> Command + Send + Sync + 'static,
        probe: Page,
        options: ProcessOptions,
    ) -> Result<(Self, mpsc::Receiver<Result<(), DevelopmentError>>), DevelopmentError> {
        if roots.is_empty() {
            return Err(DevelopmentError("source paths are required".into()));
        }
        let mut seen = std::collections::BTreeSet::new();
        for root in &roots {
            if !root.is_absolute()
                || !root
                    .canonicalize()
                    .is_ok_and(|canonical| &canonical == root)
                || (!root.is_dir() && !root.is_file())
            {
                return Err(DevelopmentError(
                    "source path must be absolute without symbolic links".into(),
                ));
            }
            if !seen.insert(root) {
                return Err(DevelopmentError("source paths contain a duplicate".into()));
            }
        }
        for path in &excluded {
            validate_excluded(path)?;
            if !roots
                .iter()
                .any(|root| root != path && path.starts_with(root))
            {
                return Err(DevelopmentError(
                    "excluded path must be below a source root".into(),
                ));
            }
        }
        options.validate()?;
        if probe.render != Render::Ssr {
            return Err(DevelopmentError("preparation requires an SSR page".into()));
        }
        process::validate_program(&build)?;
        let probe = probe
            .to_json()
            .map_err(|e| DevelopmentError(e.to_string()))?;
        let (events, events_rx) = mpsc::channel(options.event_capacity);
        let (failed, failure_rx) = watch::channel(None);
        let ignored = excluded.clone();
        let inputs = roots
            .iter()
            .map(|root| (root.clone(), root.is_dir()))
            .collect::<Vec<_>>();
        let mut watcher =
            notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                if event
                    .as_ref()
                    .is_ok_and(|event| !supervisor::input_event(event, &ignored, &inputs))
                {
                    return;
                }
                if let Err(error) = events.try_send(event) {
                    let cause = error.to_string();
                    let rejected = error.into_inner();
                    let error = DevelopmentError(format!(
                        "watch event queue failed: {cause}; rejected: {rejected:?}"
                    ));
                    tracing::error!(%error, "development watch event rejected");
                    failed.send_replace(Some(error));
                }
            })
            .map_err(|e| DevelopmentError(format!("watch creation failed: {e}")))?;
        let mut registrations = std::collections::BTreeMap::new();
        for root in &roots {
            let (path, recursive) = if root.is_dir() {
                (root.clone(), true)
            } else {
                (
                    root.parent()
                        .ok_or_else(|| DevelopmentError("source file parent is missing".into()))?
                        .to_path_buf(),
                    false,
                )
            };
            registrations
                .entry(path)
                .and_modify(|existing| *existing |= recursive)
                .or_insert(recursive);
        }
        for (path, recursive) in registrations {
            watcher
                .watch(
                    &path,
                    if recursive {
                        RecursiveMode::Recursive
                    } else {
                        RecursiveMode::NonRecursive
                    },
                )
                .map_err(|error| DevelopmentError(format!("watch registration failed: {error}")))?;
        }
        let (stop, stop_rx) = watch::channel(false);
        let current = Arc::new(RwLock::new(Err(DevelopmentError(
            "render process is preparing".into(),
        ))));
        let (changes, changes_rx) = mpsc::channel(options.event_capacity);
        let supervisor = supervisor::Supervisor::new(
            Arc::clone(&current),
            changes,
            Some((roots, excluded, build)),
            Box::new(render),
            probe,
            options,
        );
        let (worker, ready, finished) =
            supervisor.spawn(Some((events_rx, failure_rx)), stop_rx, None)?;
        let worker = wait_ready(worker, ready).await?;
        Ok((
            Self {
                current,
                watcher: Mutex::new(Some(watcher)),
                stop,
                worker: Mutex::new(Some(worker)),
                finished,
            },
            changes_rx,
        ))
    }

    pub async fn from_build(
        directory: PathBuf,
        render: impl Fn(&Path) -> Command + Send + Sync + 'static,
        probe: Page,
        options: ProcessOptions,
    ) -> Result<(Self, mpsc::Receiver<Result<(), DevelopmentError>>), DevelopmentError> {
        options.validate()?;
        if probe.render != Render::Ssr {
            return Err(DevelopmentError("preparation requires an SSR page".into()));
        }
        let probe = probe
            .to_json()
            .map_err(|e| DevelopmentError(e.to_string()))?;
        let (stop, stop_rx) = watch::channel(false);
        let current = Arc::new(RwLock::new(Err(DevelopmentError(
            "render process is preparing".into(),
        ))));
        let (changes, changes_rx) = mpsc::channel(options.event_capacity);
        let supervisor = supervisor::Supervisor::new(
            Arc::clone(&current),
            changes,
            None,
            Box::new(render),
            probe,
            options,
        );
        let (worker, ready, finished) = supervisor.spawn(None, stop_rx, Some(directory))?;
        let worker = wait_ready(worker, ready).await?;
        Ok((
            Self {
                current,
                watcher: Mutex::new(None),
                stop,
                worker: Mutex::new(Some(worker)),
                finished,
            },
            changes_rx,
        ))
    }

    pub async fn handle(
        &self,
        request: Request<Vec<u8>>,
    ) -> Result<
        Response<impl HttpBody<Data = Bytes, Error = DevelopmentError> + Send + use<>>,
        DevelopmentError,
    > {
        let generation = self
            .current
            .read()
            .map_err(|_| DevelopmentError("development state lock failed".into()))?
            .clone()?;
        let response = generation.request(request).await?;
        Ok(response.map(|incoming| body::Body::new(incoming, generation)))
    }

    pub async fn close(&self) -> Result<(), DevelopmentError> {
        self.stop.send_replace(true);
        self.watcher
            .lock()
            .map_err(|e| DevelopmentError(format!("watcher lock failed: {e}")))?
            .take();
        let mut finished = self.finished.clone();
        let result = loop {
            if let Some(result) = finished.borrow().clone() {
                break result;
            }
            finished
                .changed()
                .await
                .map_err(|e| DevelopmentError(format!("process completion unavailable: {e}")))?;
        };
        let worker = self
            .worker
            .lock()
            .map_err(|e| DevelopmentError(format!("worker lock failed: {e}")))?
            .take();
        if let Some(worker) = worker {
            return tokio::task::spawn_blocking(move || join(worker))
                .await
                .map_err(|e| DevelopmentError(format!("development join task failed: {e}")))?;
        }
        result
    }
}

impl Drop for Development {
    fn drop(&mut self) {
        self.stop.send_replace(true);
        match self.watcher.get_mut() {
            Ok(watcher) => {
                watcher.take();
            }
            Err(error) => tracing::error!(%error, "watcher lock failed during cleanup"),
        }
        match self.worker.get_mut() {
            Ok(worker) => {
                if let Some(worker) = worker.take() {
                    match join(worker) {
                        Ok(()) => tracing::info!("development cleanup completed"),
                        Err(error) => tracing::error!(%error, "development cleanup failed"),
                    }
                }
            }
            Err(error) => tracing::error!(%error, "worker lock failed during cleanup"),
        }
    }
}

fn join(worker: Worker) -> Result<(), DevelopmentError> {
    worker.join().map_err(|panic| {
        let message = panic
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| panic.downcast_ref::<&str>().copied())
            .unwrap_or("non-string panic payload");
        DevelopmentError(format!("development worker panicked: {message}"))
    })?
}

async fn wait_ready(worker: Worker, ready: Ready) -> Result<Worker, DevelopmentError> {
    match ready.await {
        Ok(Ok(())) => Ok(worker),
        result => {
            let error = match result {
                Ok(Err(error)) => error,
                Err(error) => DevelopmentError(format!("development preparation stopped: {error}")),
                Ok(Ok(())) => unreachable!(),
            };
            match tokio::task::spawn_blocking(move || join(worker)).await {
                Ok(Ok(())) => Err(error),
                Ok(Err(cleanup)) if cleanup == error => Err(error),
                Ok(Err(cleanup)) => Err(DevelopmentError(format!(
                    "{error}; cleanup failed: {cleanup}"
                ))),
                Err(cleanup) => Err(DevelopmentError(format!("{error}; join failed: {cleanup}"))),
            }
        }
    }
}
