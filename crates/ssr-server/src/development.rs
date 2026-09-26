use crate::{Adapter, Body, Server};
use http::{Request, Response};
use notify::event::EventKind;
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use ssr_build::{BuildConfig, build};
use std::fmt;
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, RwLock};
use std::thread::{self, JoinHandle};
use std::time::Duration;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DevelopmentError(String);

impl fmt::Display for DevelopmentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for DevelopmentError {}

type Current = Arc<RwLock<Result<Arc<Server>, DevelopmentError>>>;

pub struct Development {
    current: Current,
    watcher: Option<RecommendedWatcher>,
    worker: Option<JoinHandle<()>>,
}

impl Development {
    pub async fn start(
        config: BuildConfig,
        adapter: Adapter,
        worker_count: usize,
        queue_capacity: usize,
        timeout: Duration,
    ) -> Result<(Self, Receiver<Result<(), DevelopmentError>>), DevelopmentError> {
        let (events_tx, events_rx) = mpsc::channel();
        let mut watcher = notify::recommended_watcher(move |event| {
            if events_tx.send(event).is_err() {
                tracing::error!("development watch worker stopped");
            }
        })
        .map_err(|error| DevelopmentError(format!("watch creation failed: {error}")))?;
        watcher
            .watch(&config.root, RecursiveMode::Recursive)
            .map_err(|error| DevelopmentError(format!("watch registration failed: {error}")))?;
        let initial = build(&config)
            .await
            .map_err(|error| DevelopmentError(error.to_string()))?;
        let server = Server::new(&initial, adapter, worker_count, queue_capacity, timeout)
            .map_err(|error| DevelopmentError(error.to_string()))?;
        let current = Arc::new(RwLock::new(Ok(Arc::new(server))));
        let worker_current = Arc::clone(&current);
        let (changes_tx, changes_rx) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("ssr-development".into())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(error) => {
                        report(
                            &worker_current,
                            &changes_tx,
                            Err(DevelopmentError(format!("build runtime failed: {error}"))),
                        );
                        return;
                    }
                };
                while let Ok(event) = events_rx.recv() {
                    match event {
                        Ok(event) if rebuilds(&event) => {
                            let next = runtime
                                .block_on(build(&config))
                                .map_err(|error| DevelopmentError(error.to_string()))
                                .and_then(|build| {
                                    Server::new(
                                        &build,
                                        adapter,
                                        worker_count,
                                        queue_capacity,
                                        timeout,
                                    )
                                    .map(Arc::new)
                                    .map_err(|error| DevelopmentError(error.to_string()))
                                });
                            report(&worker_current, &changes_tx, next);
                        }
                        Ok(_) => {}
                        Err(error) => report(
                            &worker_current,
                            &changes_tx,
                            Err(DevelopmentError(format!("watch failed: {error}"))),
                        ),
                    }
                }
            })
            .map_err(|error| DevelopmentError(format!("watch worker failed: {error}")))?;
        Ok((
            Self {
                current,
                watcher: Some(watcher),
                worker: Some(worker),
            },
            changes_rx,
        ))
    }

    pub fn handle(&self, request: Request<Vec<u8>>) -> Result<Response<Body>, DevelopmentError> {
        let server = self
            .current
            .read()
            .map_err(|_| DevelopmentError("development server state failed".into()))?
            .clone()?;
        if self.worker.as_ref().is_some_and(JoinHandle::is_finished) {
            return Err(DevelopmentError("development worker stopped".into()));
        }
        server
            .handle(request)
            .map_err(|error| DevelopmentError(error.to_string()))
    }
}

fn rebuilds(event: &Event) -> bool {
    matches!(
        event.kind,
        EventKind::Any | EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
    )
}

fn report(
    current: &Current,
    changes: &mpsc::Sender<Result<(), DevelopmentError>>,
    next: Result<Arc<Server>, DevelopmentError>,
) {
    let result = next.as_ref().map(|_| ()).map_err(Clone::clone);
    match current.write() {
        Ok(mut guard) => *guard = next,
        Err(_) => {
            let error = DevelopmentError("development server state failed".into());
            tracing::error!(%error, "development rebuild failed");
            if changes.send(Err(error)).is_err() {
                tracing::error!("development change receiver stopped");
            }
            return;
        }
    }
    match &result {
        Ok(()) => tracing::info!("development rebuild completed"),
        Err(error) => tracing::error!(%error, "development rebuild failed"),
    }
    if changes.send(result).is_err() {
        tracing::error!("development change receiver stopped");
    }
}

impl Drop for Development {
    fn drop(&mut self) {
        self.watcher.take();
        if let Some(worker) = self.worker.take()
            && worker.join().is_err()
        {
            tracing::error!("development worker stopped unexpectedly");
        }
    }
}
