use crate::{
    Error, PoolOptions, WorkerReply, engine,
    pool::Health,
    request::{Reason, Request},
    snapshot::Snapshot,
    stream::Output,
};
use crossbeam_channel::{Receiver, Sender, bounded, select_biased};
use std::sync::{Arc, Mutex, mpsc};
use std::thread::JoinHandle;
use std::time::Instant;

pub(crate) enum Command {
    Render {
        props: String,
        state: String,
        request: Arc<Request>,
        reply: Sender<WorkerReply>,
    },
    Stream {
        props: String,
        state: String,
        nonce: String,
        output: Output,
    },
}
impl Command {
    pub(crate) fn request(&self) -> &Arc<Request> {
        match self {
            Self::Render { request, .. } => request,
            Self::Stream { output, .. } => &output.request,
        }
    }
}
pub(crate) struct Worker {
    pub(crate) command: Sender<Command>,
    threads: Mutex<Vec<(JoinHandle<()>, Receiver<()>)>>,
}

pub(crate) struct WorkerState {
    pub(crate) health: Arc<Health>,
    pub(crate) available: Sender<usize>,
    pub(crate) index: usize,
    pub(crate) watch: Sender<Arc<Request>>,
    pub(crate) options: PoolOptions,
}

impl Worker {
    pub(crate) fn new(
        snapshot: Arc<Snapshot>,
        options: PoolOptions,
        health: Arc<Health>,
        available: Sender<usize>,
        index: usize,
    ) -> Result<Self, Error> {
        let (command, requests) = bounded(0);
        let (ready, initialized) = mpsc::sync_channel(0);
        let (watch, watched) = bounded::<Arc<Request>>(1);
        let worker_health = health.clone();
        let (worker, worker_completed) = crate::worker_thread::spawn(
            format!("ssr-runtime-{index}"),
            health.clone(),
            move || {
                let state = WorkerState {
                    health: worker_health.clone(),
                    available,
                    index,
                    watch,
                    options,
                };
                let result = engine::worker(snapshot, requests, ready, state);
                if let Err(error) = result {
                    worker_health.fail(2);
                    tracing::error!(worker = index, error = %error, "render worker stopped");
                }
            },
        )
        .map_err(Error::WorkerStartup)?;
        match initialized.recv_timeout(options.timeout) {
            Ok(Ok(())) => {}
            result => {
                drop(initialized);
                health.fail(2);
                let error = match result {
                    Ok(Err(error)) => error,
                    _ => Error::WorkerStopped,
                };
                return Err(initialization_error(
                    worker,
                    worker_completed,
                    options.cleanup_timeout,
                    error,
                ));
            }
        }
        let monitor_health = health.clone();
        let monitor_result = crate::worker_thread::spawn(
            format!("ssr-runtime-watch-{index}"),
            health.clone(),
            move || monitor(watched, monitor_health, index),
        );
        let (monitor, monitor_completed) = match monitor_result {
            Ok(monitor) => monitor,
            Err(error) => {
                health.fail(2);
                let cleanup = match worker_completed.recv_timeout(options.cleanup_timeout) {
                    Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                        worker.join().map_err(|_| Error::WorkerStopped)
                    }
                    _ => Err(Error::WorkerUnresponsive),
                };
                let request = Error::WorkerStartup(error);
                return Err(match cleanup {
                    Ok(()) => request,
                    Err(cleanup) => Error::Cleanup {
                        request: Box::new(request),
                        cleanup: Box::new(cleanup),
                    },
                });
            }
        };
        Ok(Self {
            command,
            threads: Mutex::new(vec![
                (worker, worker_completed),
                (monitor, monitor_completed),
            ]),
        })
    }

    pub(crate) fn join(&self, until: Instant) -> Result<(), Error> {
        let mut threads = self.threads.lock().map_err(|_| Error::WorkerStopped)?;
        while let Some((thread, done)) = threads.pop() {
            match done.recv_timeout(until.saturating_duration_since(Instant::now())) {
                Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                    thread.join().map_err(|_| Error::WorkerStopped)?;
                }
                _ => {
                    threads.push((thread, done));
                    return Err(Error::WorkerUnresponsive);
                }
            }
        }
        Ok(())
    }
}

fn initialization_error(
    worker: JoinHandle<()>,
    completed: Receiver<()>,
    cleanup_timeout: std::time::Duration,
    error: Error,
) -> Error {
    if matches!(
        completed.recv_timeout(cleanup_timeout),
        Err(crossbeam_channel::RecvTimeoutError::Disconnected)
    ) {
        if worker.join().is_err() {
            return Error::Cleanup {
                request: Box::new(error),
                cleanup: Box::new(Error::WorkerStopped),
            };
        }
    } else {
        tracing::error!("render worker startup cleanup failed");
        return Error::Cleanup {
            request: Box::new(error),
            cleanup: Box::new(Error::WorkerUnresponsive),
        };
    }
    error
}

fn monitor(watched: Receiver<Arc<Request>>, monitor_health: Arc<Health>, index: usize) {
    loop {
        let request = select_biased! {
            recv(watched) -> request => match request {Ok(request) => request, Err(_) => break},
            recv(monitor_health.closed()) -> _ => break,
        };
        let _trace = tracing::dispatcher::set_default(&request.trace);
        let _span = request.span.enter();
        let remaining = request.deadline.saturating_duration_since(Instant::now());
        select_biased! {
            recv(request.cancellation.done()) -> _ => continue,
            recv(request.cancellation.canceled()) -> _ => {},
            recv(monitor_health.closed()) -> _ => {
                if let Err(error) = request.stop(Reason::Stopped) {
                    tracing::error!(worker = index, error = %error, "render shutdown cancellation failed");
                }
            },
            default(remaining) => {
                if let Err(error) = request.stop(Reason::Timeout) {
                    tracing::error!(worker = index, error = %error, "render timeout cancellation failed");
                }
            },
        }
        tracing::info!(
            worker = index,
            elapsed_ms = request.started.elapsed().as_secs_f64() * 1000.0,
            "render worker cancellation observed"
        );
        if let Err(error) = request.wait() {
            tracing::error!(worker = index, error = %error, "render worker requires process replacement");
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Cancellation;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn startup_error_preserves_a_failed_worker_join() {
        let (done, completed) = bounded::<()>(0);
        let worker = thread::spawn(move || {
            drop(done);
            panic!("worker cleanup failed");
        });
        let error = initialization_error(
            worker,
            completed,
            Duration::from_secs(1),
            Error::InvalidBundle("serializer failed"),
        );
        assert!(matches!(error, Error::Cleanup { request, cleanup }
            if matches!(*request, Error::InvalidBundle("serializer failed"))
                && matches!(*cleanup, Error::WorkerStopped)));
    }

    #[test]
    fn unresponsive_worker_marks_pool_unavailable_and_notifies_waiters() {
        let health = Arc::new(Health::new());
        let cancellation = Cancellation::new();
        cancellation.claim().unwrap();
        let request = Arc::new(Request {
            cancellation,
            deadline: Instant::now() + Duration::from_millis(20),
            cleanup_timeout: Duration::from_millis(30),
            health: health.clone(),
            started: Instant::now(),
            trace: tracing::dispatcher::get_default(Clone::clone),
            span: tracing::Span::none(),
        });
        let (send, recv) = bounded(1);
        send.send(request.clone()).unwrap();
        let monitor_health = health.clone();
        let monitor = thread::spawn(move || super::monitor(recv, monitor_health, 0));
        assert!(matches!(
            health.closed().recv_timeout(Duration::from_secs(1)),
            Err(crossbeam_channel::RecvTimeoutError::Disconnected)
        ));
        assert!(matches!(health.check(), Err(Error::WorkerUnresponsive)));
        monitor.join().unwrap();
        request.cancellation.finish(None).unwrap();
        request.cancellation.acknowledge().unwrap();
    }

    #[test]
    fn worker_close_reports_blocked_native_thread_and_can_join_after_cleanup() {
        let (release, released) = bounded::<()>(0);
        let (done, completed) = bounded::<()>(0);
        let native = thread::spawn(move || {
            released.recv().unwrap();
            drop(done);
        });
        let (command, _commands) = bounded(0);
        let worker = Worker {
            command,
            threads: Mutex::new(vec![(native, completed)]),
        };
        let started = Instant::now();
        assert!(matches!(
            worker.join(started + Duration::from_millis(20)),
            Err(Error::WorkerUnresponsive)
        ));
        eprintln!(
            "join returned WorkerUnresponsive with the native thread blocked after {:?}",
            started.elapsed()
        );
        assert_eq!(worker.threads.lock().unwrap().len(), 1);
        release.send(()).unwrap();
        worker
            .join(Instant::now() + Duration::from_secs(1))
            .unwrap();
        assert!(worker.threads.lock().unwrap().is_empty());
    }
}
