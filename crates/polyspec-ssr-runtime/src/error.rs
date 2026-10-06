use std::fmt;

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
    Canceled,
    LimitExceeded(&'static str),
    Cleanup {
        request: Box<Error>,
        cleanup: Box<Error>,
    },
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
            Self::Canceled => write!(f, "render canceled"),
            Self::LimitExceeded(v) => write!(f, "render limit exceeded: {v}"),
            Self::Cleanup { request, cleanup } => {
                write!(f, "{request}; render cleanup failed: {cleanup}")
            }
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
            Self::Cleanup { request, .. } => Some(request.as_ref()),
            _ => None,
        }
    }
}
