use crate::Error;
use rustix::time::{ClockId, clock_gettime};
use std::time::Duration;

/// Returns the CPU time consumed by the calling thread.
pub(crate) fn thread_time() -> Result<Duration, Error> {
    let time = clock_gettime(ClockId::ThreadCPUTime);
    let seconds = u64::try_from(time.tv_sec).map_err(|error| {
        Error::InvalidConfiguration("thread CPU time is negative", Some(Box::new(error)))
    })?;
    let nanoseconds = u32::try_from(time.tv_nsec).map_err(|error| {
        Error::InvalidConfiguration("thread CPU time is invalid", Some(Box::new(error)))
    })?;
    Ok(Duration::new(seconds, nanoseconds))
}

/// Returns the CPU time the calling thread consumed since `started`.
pub(crate) fn thread_time_since(started: Duration) -> Result<Duration, Error> {
    thread_time()?
        .checked_sub(started)
        .ok_or(Error::InvalidConfiguration(
            "thread CPU time decreased",
            None,
        ))
}
