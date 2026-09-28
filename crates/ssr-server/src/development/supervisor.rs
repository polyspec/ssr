use super::{
    Current, DevelopmentError, ProcessOptions,
    process::{self, Exit, Process},
};
use notify::Event;
use notify::event::EventKind;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinSet;

type RenderCommand = Box<dyn Fn(&Path) -> Command + Send + Sync>;

pub(super) struct Supervisor {
    current: Current,
    changes: mpsc::Sender<Result<(), DevelopmentError>>,
    roots: Vec<PathBuf>,
    excluded: Vec<PathBuf>,
    build: Option<tokio::process::Command>,
    render: RenderCommand,
    probe: Vec<u8>,
    options: ProcessOptions,
    restarts: usize,
    observer_failed: bool,
    directory: Option<PathBuf>,
    next_id: u64,
    running: JoinSet<Exit>,
    stops: BTreeMap<u64, mpsc::UnboundedSender<process::Control>>,
    errors: Vec<String>,
}

impl Supervisor {
    pub fn new(
        current: Current,
        changes: mpsc::Sender<Result<(), DevelopmentError>>,
        build: Option<(Vec<PathBuf>, Vec<PathBuf>, Command)>,
        render: RenderCommand,
        probe: Vec<u8>,
        options: ProcessOptions,
    ) -> Self {
        let (roots, excluded, build) = match build {
            Some((roots, excluded, command)) => (roots, excluded, Some(command.into())),
            None => (Vec::new(), Vec::new(), None),
        };
        Self {
            current,
            changes,
            roots,
            excluded,
            build,
            render,
            probe,
            options,
            restarts: 0,
            observer_failed: false,
            directory: None,
            next_id: 1,
            running: JoinSet::new(),
            stops: BTreeMap::new(),
            errors: Vec::new(),
        }
    }

    pub fn spawn(
        mut self,
        events: Option<super::WatchEvents>,
        cancel: watch::Receiver<bool>,
        directory: Option<PathBuf>,
    ) -> Result<super::Startup, DevelopmentError> {
        let (ready, receiver) = tokio::sync::oneshot::channel();
        let (completed, finished) = watch::channel(None);
        let worker = std::thread::Builder::new()
            .name("ssr-process-supervisor".into())
            .spawn(move || {
                let result = (|| {
                    let runtime = match tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                    {
                        Ok(runtime) => runtime,
                        Err(error) => {
                            let error = DevelopmentError(format!(
                                "process runtime creation failed: {error}"
                            ));
                            if ready.send(Err(error.clone())).is_err() {
                                tracing::error!(%error, "process preparation receiver stopped");
                            }
                            return Err(error);
                        }
                    };
                    runtime.block_on(async {
                        let result = match directory {
                            Some(directory) => self.start(directory, cancel.clone()).await,
                            None => self.rebuild(cancel.clone()).await,
                        };
                        let result = result.and_then(|()| {
                            if let Some((_, failure)) = &events
                                && let Some(error) = failure.borrow().clone()
                            {
                                return Err(error);
                            }
                            Ok(())
                        });
                        if ready.send(result.clone()).is_err() {
                            self.stop_all().await?;
                            return Err(DevelopmentError("process preparation canceled".into()));
                        }
                        if let Err(error) = result {
                            return match self.stop_all().await {
                                Ok(()) => Err(error),
                                Err(cleanup) => Err(DevelopmentError(format!(
                                    "{error}; cleanup failed: {cleanup}"
                                ))),
                            };
                        }
                        self.run(events, cancel).await
                    })
                })();
                completed.send_replace(Some(result.clone()));
                result
            })
            .map_err(|error| {
                DevelopmentError(format!("process supervisor start failed: {error}"))
            })?;
        Ok((worker, receiver, finished))
    }

    async fn rebuild(&mut self, cancel: watch::Receiver<bool>) -> Result<(), DevelopmentError> {
        let build = self.build.as_mut().ok_or_else(|| {
            DevelopmentError("prepared render process has no build command".into())
        })?;
        let directory = process::build(build, cancel.clone()).await?;
        if self
            .roots
            .iter()
            .any(|root| root.is_dir() && directory.starts_with(root))
            && !self
                .excluded
                .iter()
                .any(|excluded| directory.starts_with(excluded))
        {
            return Err(DevelopmentError(
                "build output must be outside the watched source root".into(),
            ));
        }
        self.start(directory, cancel).await?;
        self.restarts = 0;
        Ok(())
    }

    pub async fn start(
        &mut self,
        directory: PathBuf,
        cancel: watch::Receiver<bool>,
    ) -> Result<(), DevelopmentError> {
        let verified = directory.clone();
        tokio::task::spawn_blocking(move || ssr_build::Build::read(&verified))
            .await
            .map_err(|e| DevelopmentError(format!("build verification task failed: {e}")))?
            .map_err(|e| DevelopmentError(format!("build verification failed: {e}")))?;
        if *cancel.borrow() {
            return Err(DevelopmentError("render preparation canceled".into()));
        }
        let id = self.next_id;
        self.next_id = self
            .next_id
            .checked_add(1)
            .ok_or_else(|| DevelopmentError("render generation overflow".into()))?;
        let process = Process::start(
            id,
            (self.render)(&directory),
            &self.probe,
            self.options.ready_timeout,
            self.options.max_probe_bytes,
            cancel,
        )
        .await?;
        let generation = Arc::clone(&process.generation);
        self.stops.insert(id, process.stop.clone());
        self.running.spawn(process.wait(self.options.drain_timeout));
        self.directory = Some(directory);
        self.replace(Ok(generation))?;
        tracing::info!(generation = id, "render process published");
        Ok(())
    }

    fn replace(
        &self,
        value: Result<Arc<process::Generation>, DevelopmentError>,
    ) -> Result<(), DevelopmentError> {
        let previous = {
            let mut current = self
                .current
                .write()
                .map_err(|_| DevelopmentError("development state lock failed".into()))?;
            std::mem::replace(&mut *current, value)
        };
        if let Ok(previous) = previous
            && let Some(stop) = self.stops.get(&previous.id)
            && stop.send(process::Control::Retire).is_err()
        {
            tracing::debug!(
                generation = previous.id,
                "previous render process already exited"
            );
        }
        Ok(())
    }

    pub async fn run(
        mut self,
        events: Option<super::WatchEvents>,
        mut cancel: watch::Receiver<bool>,
    ) -> Result<(), DevelopmentError> {
        let (mut events, mut watch_errors) = match events {
            Some((events, errors)) => (Some(events), Some(errors)),
            None => (None, None),
        };
        loop {
            if *cancel.borrow() || self.observer_failed {
                break;
            }
            tokio::select! {
                _ = cancel.changed() => break,
                failure = async { match &mut watch_errors { Some(errors) => {
                    match errors.changed().await {
                        Ok(()) => errors.borrow().clone().ok_or_else(|| DevelopmentError("watch error event is missing".into())),
                        Err(error) => Err(DevelopmentError(format!("watch error receiver closed: {error}"))),
                    }
                }, None => std::future::pending().await } } => {
                    if *cancel.borrow() { break; }
                    let error = match failure { Ok(error) | Err(error) => error };
                    self.errors.push(error.to_string());
                    self.report(Err(error), true);
                    break;
                },
                event = async { match &mut events { Some(events) => events.recv().await, None => std::future::pending().await } } => {
                    let result = match event {
                        Some(Ok(event)) if source_event(&event, &self.excluded) => self.rebuild(cancel.clone()).await,
                        Some(Ok(_)) => continue,
                        Some(Err(error)) => Err(DevelopmentError(format!("watch failed: {error}"))),
                        None => break,
                    };
                    self.report(result, true);
                },
                exit = self.running.join_next(), if !self.running.is_empty() => {
                    let exit = match exit {
                        Some(Ok(exit)) => exit,
                        Some(Err(error)) => {
                            let failure = DevelopmentError(format!("render process supervisor failed: {error}"));
                            self.errors.push(failure.to_string());
                            self.report(Err(failure), true);
                            break;
                        }
                        None => continue,
                    };
                    self.stops.remove(&exit.id);
                    let active = match self.current.read() {
                        Ok(current) => current.as_ref().is_ok_and(|generation| generation.id == exit.id),
                        Err(error) => { self.errors.push(format!("development state lock failed: {error}")); break; }
                    };
                    if !exit.expected && active {
                        self.report(exit.result, true);
                        if self.observer_failed { continue; }
                        if self.restarts >= self.options.restart_limit {
                            self.report(Err(DevelopmentError(format!("render restart limit exhausted: {}", self.options.restart_limit))), true);
                            continue;
                        }
                        self.restarts += 1;
                        let result = match self.directory.clone() {
                            Some(directory) => self.start(directory, cancel.clone()).await,
                            None => Err(DevelopmentError("completed build directory is missing".into())),
                        };
                        self.report(result, true);
                    } else {
                        if let Err(error) = &exit.result { self.errors.push(error.to_string()); }
                        self.report(exit.result, false);
                    }
                },
            }
        }
        self.stop_all().await
    }

    fn report(&mut self, result: Result<(), DevelopmentError>, invalidate: bool) {
        if let Err(error) = &result {
            tracing::error!(%error, "development operation failed");
            if invalidate && let Err(lock) = self.replace(Err(error.clone())) {
                tracing::error!(%lock, "development state lock failed");
                self.errors.push(lock.to_string());
            }
        }
        if let Err(error) = self.changes.try_send(result) {
            let cause = error.to_string();
            let rejected = error.into_inner();
            let error = DevelopmentError(format!(
                "development change queue failed: {cause}; rejected: {rejected:?}"
            ));
            tracing::error!(%error, "development change event rejected");
            self.errors.push(error.to_string());
            self.observer_failed = true;
            if let Err(error) = self.replace(Err(error)) {
                self.errors.push(error.to_string());
            }
        }
    }

    async fn stop_all(&mut self) -> Result<(), DevelopmentError> {
        match self.current.write() {
            Ok(mut current) => {
                if current.is_ok() {
                    *current = Err(DevelopmentError("development stopped".into()));
                }
            }
            Err(error) => self
                .errors
                .push(format!("development state lock failed: {error}")),
        }
        for (id, stop) in &self.stops {
            if stop.send(process::Control::Stop).is_err() {
                tracing::debug!(generation = id, "render process already stopped");
            }
        }
        while let Some(result) = self.running.join_next().await {
            match result {
                Ok(exit) => {
                    self.stops.remove(&exit.id);
                    if let Err(error) = exit.result {
                        self.errors.push(error.to_string());
                    }
                }
                Err(error) => self
                    .errors
                    .push(format!("render process supervisor failed: {error}")),
            }
        }
        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(DevelopmentError(self.errors.join("; ")))
        }
    }
}

pub(super) fn input_event(event: &Event, excluded: &[PathBuf], inputs: &[(PathBuf, bool)]) -> bool {
    source_event(event, excluded)
        && (event.paths.is_empty()
            || event.paths.iter().any(|path| {
                !excluded.iter().any(|excluded| path.starts_with(excluded))
                    && inputs.iter().any(|(root, directory)| {
                        if *directory {
                            path.starts_with(root)
                        } else {
                            path == root
                        }
                    })
            }))
}

pub(super) fn source_event(event: &Event, excluded: &[PathBuf]) -> bool {
    matches!(
        event.kind,
        EventKind::Any | EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
    ) && (event.paths.is_empty()
        || event
            .paths
            .iter()
            .any(|path| !excluded.iter().any(|excluded| path.starts_with(excluded))))
}

#[cfg(test)]
#[path = "supervisor_tests.rs"]
mod tests;
