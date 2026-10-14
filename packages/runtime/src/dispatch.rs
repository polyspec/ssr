use crate::{Error, request::Reason, worker::Command};
use crossbeam_channel::{SendTimeoutError, Sender};
use std::time::Instant;

pub(crate) fn send(
    worker: &Sender<Command>,
    available: &Sender<usize>,
    index: usize,
    command: Command,
) -> Result<(), Error> {
    let request = command.request().clone();
    if let Err(error) = request.check() {
        return undispatched(&request, available, index, error);
    }
    match worker.send_timeout(
        command,
        request.deadline.saturating_duration_since(Instant::now()),
    ) {
        Ok(()) => Ok(()),
        Err(SendTimeoutError::Timeout(_)) => {
            request.stop(Reason::Timeout)?;
            undispatched(&request, available, index, Error::Timeout)
        }
        Err(SendTimeoutError::Disconnected(_)) => {
            request.health.fail(2);
            request.cancellation.finish(None)?;
            request.cancellation.acknowledge()?;
            request.health.check().and(Err(Error::WorkerStopped))
        }
    }
}

fn undispatched(
    request: &crate::request::Request,
    available: &Sender<usize>,
    index: usize,
    error: Error,
) -> Result<(), Error> {
    if available.send(index).is_err() {
        tracing::error!("render capacity return failed for an undispatched command");
    }
    request.cancellation.finish(None)?;
    request.cancellation.acknowledge()?;
    Err(error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Cancellation, pool::Health, request::Request};
    use crossbeam_channel::{TryRecvError, bounded};
    use std::sync::Arc;
    use std::time::Duration;

    fn command() -> (Command, Arc<Request>) {
        let cancellation = Cancellation::new();
        cancellation.claim().unwrap();
        let request = Arc::new(Request {
            cancellation,
            deadline: Instant::now() + Duration::from_millis(20),
            cleanup_timeout: Duration::from_millis(20),
            health: Arc::new(Health::new()),
            started: Instant::now(),
            trace: tracing::dispatcher::get_default(Clone::clone),
            span: tracing::Span::none(),
        });
        let (reply, _result) = bounded(1);
        (
            Command::Render {
                props: "{}".into(),
                state: "null".into(),
                request: request.clone(),
                reply,
            },
            request,
        )
    }

    #[test]
    fn deadline_before_worker_receives_command_returns_capacity_and_keeps_pool_available() {
        let (worker, _held_receiver) = bounded(0);
        let (available, capacity) = bounded(1);
        let (command, request) = command();
        assert!(matches!(
            send(&worker, &available, 7, command),
            Err(Error::Timeout)
        ));
        request.health.check().unwrap();
        assert_eq!(capacity.try_recv(), Ok(7));
        request.wait().unwrap();
    }

    #[test]
    fn cancellation_before_worker_receives_command_returns_capacity_and_keeps_pool_available() {
        let (worker, _held_receiver) = bounded(0);
        let (available, capacity) = bounded(1);
        let (command, request) = command();
        request.cancellation.cancel().unwrap();
        assert!(matches!(
            send(&worker, &available, 7, command),
            Err(Error::Canceled)
        ));
        request.health.check().unwrap();
        assert_eq!(capacity.try_recv(), Ok(7));
        request.wait().unwrap();
    }

    #[test]
    fn disconnected_worker_is_not_returned_to_available_capacity() {
        let (worker, receiver) = bounded(0);
        drop(receiver);
        let (available, capacity) = bounded(1);
        let (command, request) = command();
        assert!(matches!(
            send(&worker, &available, 7, command),
            Err(Error::WorkerStopped)
        ));
        assert!(matches!(request.health.check(), Err(Error::WorkerStopped)));
        assert_eq!(capacity.try_recv(), Err(TryRecvError::Empty));
        request.wait().unwrap();
    }
}
