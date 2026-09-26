#![forbid(unsafe_code)]

mod engine;
mod snapshot;
mod web;

use crossbeam_channel::{Receiver, Sender, TryRecvError, bounded, select};
use deno_core::v8;
use ordered_json::{Kind, Value};
use ssr_core::{Page, Render, RenderResult};
use std::fmt;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug)]
pub enum Error {
    InvalidConfiguration(&'static str),
    InvalidPage(&'static str),
    InvalidBundle(&'static str),
    InvalidResult(&'static str),
    Snapshot(&'static str),
    InvalidJson(ordered_json::Error),
    JavaScript {
        message: String,
        stack: Option<String>,
    },
    QueueFull,
    Timeout,
    WorkerStartup(std::io::Error),
    WorkerStopped,
    WorkerUnresponsive,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfiguration(v) => write!(f, "invalid runtime configuration: {v}"),
            Self::InvalidPage(v) => write!(f, "invalid page: {v}"),
            Self::InvalidBundle(v) => write!(f, "invalid server bundle: {v}"),
            Self::InvalidResult(v) => write!(f, "invalid render result: {v}"),
            Self::Snapshot(v) => write!(f, "snapshot failed: {v}"),
            Self::InvalidJson(v) => write!(f, "invalid render JSON: {v}"),
            Self::JavaScript { message, stack } => {
                write!(f, "JavaScript failed: {message}")?;
                if let Some(stack) = stack {
                    write!(f, "\n{stack}")?;
                }
                Ok(())
            }
            Self::QueueFull => write!(f, "render queue is full"),
            Self::Timeout => write!(f, "render timed out"),
            Self::WorkerStartup(v) => write!(f, "render worker initialization failed: {v}"),
            Self::WorkerStopped => write!(f, "render worker stopped"),
            Self::WorkerUnresponsive => write!(f, "render worker is unresponsive"),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidJson(v) => Some(v),
            Self::WorkerStartup(v) => Some(v),
            _ => None,
        }
    }
}

enum Command {
    Render {
        props: String,
        state: String,
        reply: Sender<WorkerReply>,
    },
    Stop,
}
struct ContextRender {
    result: RenderResult,
    context_reset: Duration,
    context_heap_delta_bytes: i128,
}
struct WorkerReply {
    result: Result<ContextRender, Error>,
    heap_used_bytes: usize,
}
struct Worker {
    command: Sender<Command>,
    handle: v8::IsolateHandle,
    thread: Option<thread::JoinHandle<()>>,
    done: Receiver<()>,
    poisoned: AtomicBool,
}
impl Drop for Worker {
    fn drop(&mut self) {
        if self.poisoned.load(Ordering::Acquire) {
            let _ = self.command.try_send(Command::Stop);
            if self.done.try_recv().is_ok()
                && let Some(thread) = self.thread.take()
            {
                let _ = thread.join();
            }
            return;
        } else {
            let _ = self
                .command
                .send_timeout(Command::Stop, Duration::from_secs(1));
        }
        if self.done.recv_timeout(Duration::from_secs(1)).is_ok()
            && let Some(thread) = self.thread.take()
        {
            let _ = thread.join();
        }
    }
}

pub struct Pool {
    workers: Vec<Worker>,
    available_tx: Sender<usize>,
    available_rx: Receiver<usize>,
    waiting: AtomicUsize,
    healthy: AtomicUsize,
    loss_reason: AtomicU8,
    closed_tx: Mutex<Option<Sender<()>>>,
    closed_rx: Receiver<()>,
    queue_capacity: usize,
    timeout: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderMetrics {
    pub pool_wait: Duration,
    pub heap_used_bytes: usize,
    pub context_reset: Duration,
    pub context_heap_delta_bytes: i128,
}

struct Lease<'a> {
    pool: &'a Pool,
    index: usize,
    release: bool,
}
impl Drop for Lease<'_> {
    fn drop(&mut self) {
        if self.release {
            let _ = self.pool.available_tx.send(self.index);
        }
    }
}

impl Pool {
    pub fn new(
        bundle: Vec<u8>,
        worker_count: usize,
        queue_capacity: usize,
        timeout: Duration,
    ) -> Result<Self, Error> {
        if worker_count == 0 {
            return Err(Error::InvalidConfiguration("worker count must be nonzero"));
        }
        if timeout.is_zero() {
            return Err(Error::InvalidConfiguration("timeout must be nonzero"));
        }
        let bundle = std::str::from_utf8(&bundle)
            .map_err(|_| Error::InvalidBundle("UTF-8 required"))?
            .to_owned();
        if bundle.is_empty() {
            return Err(Error::InvalidBundle("bundle is empty"));
        }
        let snapshot = snapshot::get(&bundle, timeout)?;
        let (available_tx, available_rx) = bounded(worker_count);
        let (closed_tx, closed_rx) = bounded(0);
        let mut workers = Vec::with_capacity(worker_count);
        for index in 0..worker_count {
            let (command, requests) = bounded::<Command>(0);
            let (ready_tx, ready_rx) = mpsc::sync_channel(0);
            let (done_tx, done_rx) = bounded(1);
            let snapshot = snapshot.clone();
            let join = thread::Builder::new()
                .name(format!("ssr-runtime-{index}"))
                .spawn(move || {
                    engine::worker(snapshot, requests, ready_tx);
                    let _ = done_tx.send(());
                })
                .map_err(Error::WorkerStartup)?;
            let handle = match ready_rx.recv_timeout(timeout) {
                Ok(Ok(handle)) => handle,
                Ok(Err(error)) => {
                    if done_rx.recv_timeout(Duration::from_secs(1)).is_ok() {
                        let _ = join.join();
                    }
                    return Err(error);
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    if done_rx.recv_timeout(Duration::from_secs(1)).is_ok() {
                        let _ = join.join();
                    }
                    return Err(Error::WorkerStopped);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => return Err(Error::WorkerUnresponsive),
            };
            workers.push(Worker {
                command,
                handle,
                thread: Some(join),
                done: done_rx,
                poisoned: AtomicBool::new(false),
            });
            available_tx.send(index).map_err(|_| Error::WorkerStopped)?;
        }
        Ok(Self {
            workers,
            available_tx,
            available_rx,
            waiting: AtomicUsize::new(0),
            healthy: AtomicUsize::new(worker_count),
            loss_reason: AtomicU8::new(0),
            closed_tx: Mutex::new(Some(closed_tx)),
            closed_rx,
            queue_capacity,
            timeout,
        })
    }

    pub fn render(&self, page: &Page) -> Result<RenderResult, Error> {
        self.render_with_metrics(page).map(|(result, _)| result)
    }

    pub fn render_with_metrics(&self, page: &Page) -> Result<(RenderResult, RenderMetrics), Error> {
        if page.render != Render::Ssr {
            return Err(Error::InvalidPage("runtime requires SSR"));
        }
        if page.props.kind() != Kind::Object {
            return Err(Error::InvalidPage("props must be an object"));
        }
        let props = page.props.compact();
        let state = page.state.compact();
        let deadline = Instant::now()
            .checked_add(self.timeout)
            .ok_or(Error::InvalidConfiguration("timeout exceeds clock range"))?;
        let waiting_since = Instant::now();
        let mut lease = self.acquire(deadline)?;
        let pool_wait = waiting_since.elapsed();
        let (reply_tx, reply_rx) = bounded(1);
        let worker = &self.workers[lease.index];
        let command = Command::Render {
            props,
            state,
            reply: reply_tx,
        };
        match worker
            .command
            .send_timeout(command, deadline.saturating_duration_since(Instant::now()))
        {
            Ok(()) => {}
            Err(crossbeam_channel::SendTimeoutError::Timeout(_)) => {
                self.mark_lost(&mut lease, 2);
                return Err(Error::WorkerUnresponsive);
            }
            Err(crossbeam_channel::SendTimeoutError::Disconnected(_)) => {
                self.mark_lost(&mut lease, 1);
                return Err(Error::WorkerStopped);
            }
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        match reply_rx.recv_timeout(remaining) {
            Ok(WorkerReply {
                result,
                heap_used_bytes,
            }) => result.map(
                |ContextRender {
                     result,
                     context_reset,
                     context_heap_delta_bytes,
                 }| {
                    (
                        result,
                        RenderMetrics {
                            pool_wait,
                            heap_used_bytes,
                            context_reset,
                            context_heap_delta_bytes,
                        },
                    )
                },
            ),
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                if !worker.handle.terminate_execution() {
                    self.mark_lost(&mut lease, 1);
                    return Err(Error::WorkerStopped);
                }
                // V8 stops JavaScript, but it cannot interrupt a native callback.
                match reply_rx.recv_timeout(Duration::from_secs(1)) {
                    Ok(_) => Err(Error::Timeout),
                    Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                        self.mark_lost(&mut lease, 1);
                        Err(Error::WorkerStopped)
                    }
                    Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                        self.mark_lost(&mut lease, 2);
                        Err(Error::WorkerUnresponsive)
                    }
                }
            }
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                self.mark_lost(&mut lease, 1);
                Err(Error::WorkerStopped)
            }
        }
    }

    fn lost_error(&self) -> Error {
        if self.loss_reason.load(Ordering::Acquire) == 2 {
            Error::WorkerUnresponsive
        } else {
            Error::WorkerStopped
        }
    }

    fn mark_lost(&self, lease: &mut Lease<'_>, reason: u8) {
        lease.release = false;
        let worker = &self.workers[lease.index];
        if worker.poisoned.swap(true, Ordering::AcqRel) {
            return;
        }
        if self.healthy.fetch_sub(1, Ordering::AcqRel) == 1 {
            self.loss_reason.store(reason, Ordering::Release);
            self.closed_tx
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take();
        }
    }

    fn acquire(&self, deadline: Instant) -> Result<Lease<'_>, Error> {
        if self.healthy.load(Ordering::Acquire) == 0 {
            return Err(self.lost_error());
        }
        match self.available_rx.try_recv() {
            Ok(index) => {
                return Ok(Lease {
                    pool: self,
                    index,
                    release: true,
                });
            }
            Err(TryRecvError::Disconnected) => return Err(Error::WorkerStopped),
            Err(TryRecvError::Empty) => {}
        }
        self.waiting
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                (n < self.queue_capacity).then_some(n + 1)
            })
            .map_err(|_| Error::QueueFull)?;
        let timeout = crossbeam_channel::after(deadline.saturating_duration_since(Instant::now()));
        let result = select! {
            recv(self.available_rx) -> result => result.map_err(|_| Error::WorkerStopped),
            recv(self.closed_rx) -> _ => Err(self.lost_error()),
            recv(timeout) -> _ => Err(Error::Timeout),
        };
        self.waiting.fetch_sub(1, Ordering::AcqRel);
        result.map(|index| Lease {
            pool: self,
            index,
            release: true,
        })
    }
}

pub(crate) fn state_from_json(bytes: &[u8]) -> Result<Value, Error> {
    ordered_json::parse_bytes_reject_duplicates(bytes).map_err(Error::InvalidJson)
}

#[cfg(test)]
mod tests {
    use super::*;
    use deno_core::{JsRuntime, RuntimeOptions};
    use std::sync::Arc;

    fn unresponsive_pool(timeout: Duration) -> (Pool, Sender<()>, Receiver<Command>) {
        let scheduler = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let _entered = scheduler.enter();
        let mut runtime = JsRuntime::new(RuntimeOptions::default());
        let handle = runtime.v8_isolate().thread_safe_handle();
        let (command, requests) = bounded(0);
        let (done_tx, done) = bounded(1);
        let (release, released) = bounded(0);
        let thread = thread::spawn(move || {
            let _ = released.recv();
            let _ = done_tx.send(());
        });
        let (available_tx, available_rx) = bounded(1);
        available_tx.send(0).unwrap();
        let (closed_tx, closed_rx) = bounded(0);
        let pool = Pool {
            workers: vec![Worker {
                command,
                handle,
                thread: Some(thread),
                done,
                poisoned: AtomicBool::new(false),
            }],
            available_tx,
            available_rx,
            waiting: AtomicUsize::new(0),
            healthy: AtomicUsize::new(1),
            loss_reason: AtomicU8::new(0),
            closed_tx: Mutex::new(Some(closed_tx)),
            closed_rx,
            queue_capacity: 1,
            timeout,
        };
        (pool, release, requests)
    }

    fn page() -> Page {
        Page::from_json(br#"{"render":"ssr","title":"T","language":"en","props":{},"state":null}"#)
            .unwrap()
    }

    #[test]
    fn unresponsive_worker_is_removed_and_drop_does_not_wait_for_thread() {
        let (pool, release, _requests) = unresponsive_pool(Duration::from_millis(50));
        let start = Instant::now();
        assert!(matches!(
            pool.render(&page()),
            Err(Error::WorkerUnresponsive)
        ));
        assert!(matches!(
            pool.render(&page()),
            Err(Error::WorkerUnresponsive)
        ));
        drop(pool);
        assert!(start.elapsed() < Duration::from_millis(500));
        release.send(()).unwrap();
    }

    #[test]
    fn waiting_call_wakes_when_last_worker_is_lost() {
        let (pool, release, _requests) = unresponsive_pool(Duration::from_secs(2));
        let pool = Arc::new(pool);
        let mut lease = pool
            .acquire(Instant::now() + Duration::from_secs(2))
            .unwrap();
        let waiting_pool = Arc::clone(&pool);
        let (started, ready) = bounded(0);
        let waiting = thread::spawn(move || {
            started.send(()).unwrap();
            waiting_pool.render(&page())
        });
        ready.recv().unwrap();
        let start = Instant::now();
        pool.mark_lost(&mut lease, 2);
        drop(lease);
        assert!(matches!(
            waiting.join().unwrap(),
            Err(Error::WorkerUnresponsive)
        ));
        assert!(start.elapsed() < Duration::from_millis(500));
        drop(pool);
        release.send(()).unwrap();
    }

    #[test]
    fn timeout_covers_queue_wait_and_execution() {
        let pool = Arc::new(
            Pool::new(
                b"function render(props, state) { const until = Date.now() + 1800; while (Date.now() < until) {} return {html:'ok', state}; }".to_vec(),
                1,
                1,
                Duration::from_secs(2),
            )
            .unwrap(),
        );
        let lease = pool
            .acquire(Instant::now() + Duration::from_secs(2))
            .unwrap();
        let waiting_pool = Arc::clone(&pool);
        let waiting = thread::spawn(move || waiting_pool.render(&page()));
        let observed_by = Instant::now() + Duration::from_secs(5);
        while pool.waiting.load(Ordering::Acquire) == 0 {
            assert!(
                Instant::now() < observed_by,
                "render did not enter the queue"
            );
            thread::yield_now();
        }
        thread::sleep(Duration::from_millis(500));
        assert_eq!(
            pool.waiting.load(Ordering::Acquire),
            1,
            "render timed out before the worker was released"
        );
        drop(lease);
        let result = waiting.join().unwrap();
        assert!(matches!(result, Err(Error::Timeout)), "{result:?}");
    }
}
