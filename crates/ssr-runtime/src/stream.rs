use crate::{
    Cancellation, Error,
    request::{Reason, Request},
};
use crossbeam_channel::{Receiver, Sender, after, select_biased};
use ordered_json::Value;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub(crate) enum Event {
    Shell(Value, usize),
    Chunk(Vec<u8>),
    End,
    Failed(Error),
}

pub(crate) struct Output {
    pub(crate) events: Sender<Event>,
    pub(crate) request: Arc<Request>,
    pub(crate) max_chunk_bytes: usize,
    pub(crate) max_output_bytes: usize,
}
impl Output {
    pub(crate) fn send(&self, event: Event) -> Result<(), Error> {
        self.request.check()?;
        let timeout = after(
            self.request
                .deadline
                .saturating_duration_since(Instant::now()),
        );
        select_biased! {
            recv(self.request.cancellation.canceled()) -> _ => {
                Err(self.request.cancellation.failure()?)
            },
            recv(timeout) -> _ => { self.request.stop(Reason::Timeout)?; Err(Error::Timeout) },
            send(self.events, event) -> result => result.map_err(|_| Error::Canceled),
        }
    }
}

pub struct Stream {
    pub(crate) events: Receiver<Event>,
    pub(crate) request: Arc<Request>,
    pub(crate) metrics: StreamMetrics,
    pub(crate) finished: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamMetrics {
    pub pool_wait: Duration,
    pub heap_used_bytes: usize,
}

impl Stream {
    pub fn metrics(&self) -> StreamMetrics {
        self.metrics
    }

    pub fn cancel_handle(&self) -> Cancellation {
        self.request.cancellation.clone()
    }

    pub fn close(&mut self) -> Result<(), Error> {
        self.request.stop(Reason::Canceled)?;
        self.finished = true;
        self.request.wait()
    }

    pub(crate) fn receive(&self) -> Result<Event, Error> {
        self.request.check()?;
        let timeout = after(
            self.request
                .deadline
                .saturating_duration_since(Instant::now()),
        );
        select_biased! {
            recv(self.request.cancellation.canceled()) -> _ => {
                Err(self.request.cancellation.failure()?)
            },
            recv(self.events) -> result => result.map_err(|_| Error::WorkerStopped),
            recv(timeout) -> _ => {self.request.stop(Reason::Timeout)?; Err(Error::Timeout)},
        }
    }

    fn fail(&mut self, error: Error) -> Option<Result<Vec<u8>, Error>> {
        self.finished = true;
        if let Err(cleanup) = self
            .request
            .stop(Reason::Canceled)
            .and_then(|_| self.request.wait())
        {
            return Some(Err(Error::Cleanup {
                request: Box::new(error),
                cleanup: Box::new(cleanup),
            }));
        }
        Some(Err(error))
    }
}

impl Iterator for Stream {
    type Item = Result<Vec<u8>, Error>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }
        match self.receive() {
            Ok(Event::Chunk(chunk)) => Some(Ok(chunk)),
            Ok(Event::End) => {
                self.finished = true;
                self.request.wait().err().map(Err)
            }
            Ok(Event::Failed(error)) | Err(error) => self.fail(error),
            Ok(Event::Shell(_, _)) => self.fail(Error::InvalidResult("duplicate stream shell")),
        }
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        if !self.finished
            && let Err(error) = self.request.stop(Reason::Canceled)
        {
            tracing::error!(error = %error, "render stream cancellation failed");
        }
    }
}
