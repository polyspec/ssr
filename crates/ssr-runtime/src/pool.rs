use crate::{
    Cancellation, ContextRender, Error, PoolOptions, RenderMetrics, Stream, StreamMetrics,
    WorkerReply,
    request::{Reason, Request},
    snapshot::Snapshot,
    stream::{Event, Output},
    worker::{Command, Worker},
};
use crossbeam_channel::{Receiver, Sender, TryRecvError, bounded, select_biased};
use ordered_json::{Kind, Value};
use ssr_core::{Page, Render, RenderResult};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU8, Ordering},
};
use std::time::Instant;

pub(crate) struct Health {
    status: AtomicU8,
    stop: Mutex<Option<Sender<()>>>,
    stopped: Receiver<()>,
}
impl Health {
    pub fn new() -> Self {
        let (stop, stopped) = bounded(0);
        Self {
            status: AtomicU8::new(0),
            stop: Mutex::new(Some(stop)),
            stopped,
        }
    }
    pub(crate) fn fail(&self, reason: u8) {
        self.status.fetch_max(reason, Ordering::AcqRel);
        match self.stop.lock() {
            Ok(mut stop) => {
                stop.take();
            }
            Err(_) => {
                tracing::error!("render pool shutdown state is poisoned");
            }
        }
    }
    pub(crate) fn check(&self) -> Result<(), Error> {
        match self.status.load(Ordering::Acquire) {
            0 => Ok(()),
            3 => Err(Error::WorkerUnresponsive),
            _ => Err(Error::WorkerStopped),
        }
    }
    pub(crate) fn closed(&self) -> &Receiver<()> {
        &self.stopped
    }
}

#[derive(Default)]
struct Waiting {
    count: usize,
    bytes: usize,
}
struct Admission<'a> {
    waiting: &'a Mutex<Waiting>,
    bytes: usize,
}
impl Drop for Admission<'_> {
    fn drop(&mut self) {
        match self.waiting.lock() {
            Ok(mut waiting) => {
                waiting.count -= 1;
                waiting.bytes -= self.bytes;
            }
            Err(_) => tracing::error!("render queue state is poisoned"),
        }
    }
}
#[cfg(test)]
#[derive(Default)]
pub(crate) struct QueueEntry {
    pub(crate) delay: std::time::Duration,
    pub(crate) entered: Option<Sender<()>>,
}
pub struct Pool {
    pub(crate) workers: Vec<Worker>,
    available_tx: Sender<usize>,
    available_rx: Receiver<usize>,
    waiting: Mutex<Waiting>,
    pub(crate) health: Arc<Health>,
    options: PoolOptions,
    #[cfg(test)]
    pub(crate) queue_entry: Mutex<QueueEntry>,
}

impl Pool {
    pub(crate) fn from_snapshot(
        snapshot: Arc<Snapshot>,
        options: PoolOptions,
    ) -> Result<Self, Error> {
        let (available_tx, available_rx) = bounded(options.worker_count);
        let mut pool = Self {
            workers: Vec::with_capacity(options.worker_count),
            available_tx,
            available_rx,
            waiting: Mutex::new(Waiting::default()),
            health: Arc::new(Health::new()),
            options,
            #[cfg(test)]
            queue_entry: Mutex::new(QueueEntry::default()),
        };
        for index in 0..options.worker_count {
            let worker = Worker::new(
                snapshot.clone(),
                options,
                pool.health.clone(),
                pool.available_tx.clone(),
                index,
            )?;
            pool.workers.push(worker);
            pool.available_tx
                .send(index)
                .map_err(|_| Error::WorkerStopped)?;
        }
        Ok(pool)
    }

    pub fn health(&self) -> Result<(), Error> {
        self.health.check()
    }

    pub fn wait(&self) -> Result<(), Error> {
        match self.health.closed().recv() {
            Err(_) => self.health(),
            Ok(()) => Err(Error::InvalidConfiguration(
                "pool closure signal contained a value",
            )),
        }
    }

    pub fn close(&self) -> Result<(), Error> {
        self.health.fail(1);
        let until = Instant::now() + self.options.cleanup_timeout;
        let mut failure = None;
        for worker in &self.workers {
            if let Err(error) = worker.join(until) {
                self.health
                    .fail(if matches!(error, Error::WorkerUnresponsive) {
                        3
                    } else {
                        2
                    });
                tracing::error!(error = %error, "render worker close failed");
                failure = Some(error);
            }
        }
        failure.map_or(Ok(()), Err)
    }

    fn request(&self, cancellation: Cancellation, nonce: &str) -> Result<Arc<Request>, Error> {
        self.health()?;
        cancellation.claim()?;
        Ok(Arc::new(Request {
            cancellation,
            deadline: Instant::now() + self.options.timeout,
            cleanup_timeout: self.options.cleanup_timeout,
            health: self.health.clone(),
            started: Instant::now(),
            trace: tracing::dispatcher::get_default(Clone::clone),
            span: tracing::info_span!("render", nonce),
        }))
    }

    fn inputs(&self, page: &Page, nonce: &str) -> Result<(String, String), Error> {
        if page.render != Render::Ssr || page.props.kind() != Kind::Object {
            return Err(Error::InvalidPage("runtime requires SSR and object props"));
        }
        let props = page.props.compact();
        let state = page.state.compact();
        let size = props
            .len()
            .checked_add(state.len())
            .and_then(|size| size.checked_add(nonce.len()))
            .ok_or(Error::LimitExceeded("input bytes"))?;
        if size > self.options.max_input_bytes {
            return Err(Error::LimitExceeded("input bytes"));
        }
        Ok((props, state))
    }

    fn acquire(&self, request: &Request, input_bytes: usize) -> Result<usize, Error> {
        self.health()?;
        request.check()?;
        match self.available_rx.try_recv() {
            Ok(index) => return Ok(index),
            Err(TryRecvError::Disconnected) => return Err(Error::WorkerStopped),
            Err(TryRecvError::Empty) => {}
        }
        #[cfg(test)]
        let entered = {
            let mut entry = self.queue_entry.lock().map_err(|_| Error::WorkerStopped)?;
            let delay = entry.delay;
            let entered = entry.entered.take();
            drop(entry);
            std::thread::sleep(delay);
            entered
        };
        {
            let mut waiting = self.waiting.lock().map_err(|_| Error::WorkerStopped)?;
            if waiting.count >= self.options.queue_capacity {
                return Err(Error::QueueFull);
            }
            let bytes = waiting
                .bytes
                .checked_add(input_bytes)
                .ok_or(Error::LimitExceeded("queue bytes"))?;
            if bytes > self.options.max_queue_bytes {
                return Err(Error::LimitExceeded("queue bytes"));
            }
            waiting.count += 1;
            waiting.bytes = bytes;
        }
        let _admission = Admission {
            waiting: &self.waiting,
            bytes: input_bytes,
        };
        #[cfg(test)]
        if let Some(entered) = entered {
            entered.send(()).map_err(|_| {
                Error::InvalidConfiguration("queue entry signal receiver is closed")
            })?;
        }
        select_biased! {
            recv(request.cancellation.canceled()) -> _ => Err(request.cancellation.failure()?),
            recv(self.health.closed()) -> _ => self.health().and(Err(Error::WorkerStopped)),
            recv(self.available_rx) -> value => value.map_err(|_| Error::WorkerStopped),
            default(request.deadline.saturating_duration_since(Instant::now())) => Err(Error::Timeout),
        }
    }

    fn dispatch(&self, command: Command, input_bytes: usize) -> Result<std::time::Duration, Error> {
        let started = Instant::now();
        let request = command.request().clone();
        let index = match self.acquire(&request, input_bytes) {
            Ok(index) => index,
            Err(error) => {
                request.cancellation.finish(None)?;
                request.cancellation.acknowledge()?;
                return Err(error);
            }
        };
        let wait = started.elapsed();
        crate::dispatch::send(
            &self.workers[index].command,
            &self.available_tx,
            index,
            command,
        )?;
        Ok(wait)
    }

    pub fn render(&self, page: &Page) -> Result<RenderResult, Error> {
        self.render_with_metrics(page).map(|(result, _)| result)
    }

    pub fn render_with_metrics(&self, page: &Page) -> Result<(RenderResult, RenderMetrics), Error> {
        let (props, state) = self.inputs(page, "")?;
        let bytes = props.len() + state.len();
        let request = self.request(Cancellation::new(), "")?;
        let (reply, result) = bounded(1);
        let pool_wait = self.dispatch(
            Command::Render {
                props,
                state,
                request: request.clone(),
                reply,
            },
            bytes,
        )?;
        let received = select_biased! {
            recv(result) -> result => result.map_err(|_| Error::WorkerStopped),
            recv(request.cancellation.canceled()) -> _ => Err(request.cancellation.failure()?),
            default(request.deadline.saturating_duration_since(Instant::now())) => {request.stop(Reason::Timeout)?; Err(Error::Timeout)},
        };
        if let Err(cleanup) = request.wait() {
            return Err(match received {
                Err(error)
                | Ok(WorkerReply {
                    result: Err(error), ..
                }) => Error::Cleanup {
                    request: Box::new(error),
                    cleanup: Box::new(cleanup),
                },
                Ok(_) => cleanup,
            });
        }
        let WorkerReply {
            result,
            heap_used_bytes,
        } = received?;
        let ContextRender {
            result,
            context_reset,
            context_heap_delta_bytes,
            #[cfg(feature = "bench")]
            render_cpu,
        } = result?;
        Ok((
            result,
            RenderMetrics {
                pool_wait,
                heap_used_bytes,
                context_reset,
                context_heap_delta_bytes,
                #[cfg(feature = "bench")]
                render_cpu,
            },
        ))
    }

    pub fn render_stream(
        &self,
        page: &Page,
        nonce: &str,
        cancellation: Cancellation,
    ) -> Result<(Value, Stream), Error> {
        if nonce.is_empty()
            || !nonce
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
        {
            return Err(Error::InvalidPage("nonce must be nonempty URL-safe text"));
        }
        let (props, state) = self.inputs(page, nonce)?;
        let bytes = props.len() + state.len() + nonce.len();
        let request = self.request(cancellation, nonce)?;
        let (events, received) = bounded(1);
        let output = Output {
            events,
            request: request.clone(),
            max_chunk_bytes: self.options.max_chunk_bytes,
            max_output_bytes: self.options.max_output_bytes,
        };
        let pool_wait = self.dispatch(
            Command::Stream {
                props,
                state,
                nonce: nonce.to_owned(),
                output,
            },
            bytes,
        )?;
        let mut stream = Stream {
            events: received,
            request,
            metrics: StreamMetrics {
                pool_wait,
                heap_used_bytes: 0,
            },
            finished: false,
        };
        match stream.receive() {
            Ok(Event::Shell(state, heap)) => {
                stream.metrics.heap_used_bytes = heap;
                Ok((state, stream))
            }
            result => {
                let error = match result {
                    Ok(Event::Failed(error)) | Err(error) => error,
                    _ => Error::InvalidResult("stream shell required before body"),
                };
                if let Err(cleanup) = stream.close() {
                    return Err(Error::Cleanup {
                        request: Box::new(error),
                        cleanup: Box::new(cleanup),
                    });
                }
                Err(error)
            }
        }
    }
}

impl Drop for Pool {
    fn drop(&mut self) {
        if let Err(error) = self.close() {
            tracing::error!(error = %error, "render pool cleanup requires process replacement");
        }
    }
}
