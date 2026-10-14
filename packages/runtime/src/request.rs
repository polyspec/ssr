use crate::{Error, pool::Health};
use crossbeam_channel::{Receiver, Sender, bounded};
use deno_core::v8;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone, Copy)]
pub(crate) enum Reason {
    Canceled,
    Timeout,
    Stopped,
}
impl Reason {
    pub(crate) fn error(self) -> Error {
        match self {
            Self::Canceled => Error::Canceled,
            Self::Timeout => Error::Timeout,
            Self::Stopped => Error::WorkerStopped,
        }
    }
}
enum Phase {
    New,
    Waiting,
    Running(v8::IsolateHandle),
    Complete,
}
struct State {
    phase: Phase,
    reason: Option<Reason>,
    cancel: Option<Sender<()>>,
    done: Option<Sender<()>>,
}
struct Inner {
    state: Mutex<State>,
    canceled: Receiver<()>,
    done: Receiver<()>,
}

#[derive(Clone)]
pub struct Cancellation(Arc<Inner>);

impl Default for Cancellation {
    fn default() -> Self {
        Self::new()
    }
}
impl Cancellation {
    pub fn new() -> Self {
        let (cancel, canceled) = bounded(0);
        let (done, completed) = bounded(0);
        Self(Arc::new(Inner {
            state: Mutex::new(State {
                phase: Phase::New,
                reason: None,
                cancel: Some(cancel),
                done: Some(done),
            }),
            canceled,
            done: completed,
        }))
    }

    pub fn cancel(&self) -> Result<(), Error> {
        self.stop(Reason::Canceled)
    }

    pub(crate) fn stop(&self, reason: Reason) -> Result<(), Error> {
        let mut state = self.0.state.lock().map_err(|_| Error::WorkerStopped)?;
        if matches!(state.phase, Phase::Complete) || state.reason.is_some() {
            return Ok(());
        }
        state.reason = Some(reason);
        state.cancel.take();
        if let Phase::Running(handle) = &state.phase
            && !handle.terminate_execution()
        {
            return Err(Error::WorkerStopped);
        }
        Ok(())
    }

    pub(crate) fn claim(&self) -> Result<(), Error> {
        let mut state = self.0.state.lock().map_err(|_| Error::WorkerStopped)?;
        if !matches!(state.phase, Phase::New) {
            return Err(Error::InvalidConfiguration(
                "cancellation belongs to one request",
            ));
        }
        state.phase = Phase::Waiting;
        match state.reason {
            Some(reason) => Err(reason.error()),
            None => Ok(()),
        }
    }

    pub(crate) fn start(&self, handle: v8::IsolateHandle) -> Result<(), Error> {
        let mut state = self.0.state.lock().map_err(|_| Error::WorkerStopped)?;
        if let Some(reason) = state.reason {
            return Err(reason.error());
        }
        state.phase = Phase::Running(handle);
        Ok(())
    }

    pub(crate) fn finish(&self, runtime: Option<&mut v8::OwnedIsolate>) -> Result<(), Error> {
        let mut state = self.0.state.lock().map_err(|_| Error::WorkerStopped)?;
        if let Some(runtime) = runtime {
            runtime.cancel_terminate_execution();
        }
        state.phase = Phase::Complete;
        Ok(())
    }

    pub(crate) fn acknowledge(&self) -> Result<(), Error> {
        self.0
            .state
            .lock()
            .map_err(|_| Error::WorkerStopped)?
            .done
            .take();
        Ok(())
    }

    pub(crate) fn error(&self) -> Result<Option<Error>, Error> {
        Ok(self
            .0
            .state
            .lock()
            .map_err(|_| Error::WorkerStopped)?
            .reason
            .map(Reason::error))
    }

    pub(crate) fn failure(&self) -> Result<Error, Error> {
        self.error()?
            .ok_or(Error::InvalidConfiguration("cancellation reason is absent"))
    }

    pub(crate) fn canceled(&self) -> &Receiver<()> {
        &self.0.canceled
    }

    pub(crate) fn done(&self) -> &Receiver<()> {
        &self.0.done
    }
}

pub(crate) struct Request {
    pub(crate) cancellation: Cancellation,
    pub(crate) deadline: Instant,
    pub(crate) cleanup_timeout: Duration,
    pub(crate) health: Arc<Health>,
    pub(crate) started: Instant,
    pub(crate) trace: tracing::Dispatch,
    pub(crate) span: tracing::Span,
}
impl Request {
    pub(crate) fn stop(&self, reason: Reason) -> Result<(), Error> {
        if let Err(error) = self.cancellation.stop(reason) {
            self.health.fail(2);
            return Err(error);
        }
        Ok(())
    }
    pub(crate) fn wait(&self) -> Result<(), Error> {
        match self.cancellation.done().recv_timeout(self.cleanup_timeout) {
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => Ok(()),
            _ => {
                self.health.fail(3);
                Err(Error::WorkerUnresponsive)
            }
        }
    }
    pub(crate) fn check(&self) -> Result<(), Error> {
        if let Some(error) = self.cancellation.error()? {
            return Err(error);
        }
        if Instant::now() >= self.deadline {
            self.stop(Reason::Timeout)?;
            return Err(Error::Timeout);
        }
        Ok(())
    }
}
