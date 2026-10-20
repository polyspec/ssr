use std::fmt;

/// The original error that a runtime error keeps as its source, when one exists.
type Source = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug)]
pub enum Error {
    InvalidConfiguration(&'static str, Option<Source>),
    InvalidPage(&'static str),
    InvalidBundle(&'static str, Option<Source>),
    InvalidResult(&'static str, Option<Source>),
    Snapshot(&'static str, Option<Source>),
    InvalidJson(ordered_json::Error),
    JavaScript {
        message: String,
        stack: Option<String>,
    },
    QueueFull,
    Timeout,
    Canceled(Option<Source>),
    LimitExceeded(&'static str),
    Cleanup {
        request: Box<Error>,
        cleanup: Box<Error>,
    },
    WorkerStartup(std::io::Error),
    WorkerStopped(Option<Source>),
    WorkerUnresponsive,
}

/// Writes the reason of a variant and, when it keeps its original error, that error after a colon.
fn write_reason(
    f: &mut fmt::Formatter<'_>,
    prefix: &str,
    reason: &str,
    source: Option<&Source>,
) -> fmt::Result {
    write!(f, "{prefix}: {reason}")?;
    match source {
        Some(source) => write!(f, ": {source}"),
        None => Ok(()),
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfiguration(v, source) => {
                write_reason(f, "invalid runtime configuration", v, source.as_ref())
            }
            Self::InvalidPage(v) => write!(f, "invalid page: {v}"),
            Self::InvalidBundle(v, source) => {
                write_reason(f, "invalid server bundle", v, source.as_ref())
            }
            Self::InvalidResult(v, source) => {
                write_reason(f, "invalid render result", v, source.as_ref())
            }
            Self::Snapshot(v, source) => write_reason(f, "snapshot failed", v, source.as_ref()),
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
            Self::Canceled(source) => match source {
                Some(source) => write!(f, "render canceled: {source}"),
                None => write!(f, "render canceled"),
            },
            Self::LimitExceeded(v) => write!(f, "render limit exceeded: {v}"),
            Self::Cleanup { request, cleanup } => {
                write!(f, "{request}; render cleanup failed: {cleanup}")
            }
            Self::WorkerStartup(v) => write!(f, "render worker initialization failed: {v}"),
            Self::WorkerStopped(source) => match source {
                Some(source) => write!(f, "render worker stopped: {source}"),
                None => write!(f, "render worker stopped"),
            },
            Self::WorkerUnresponsive => write!(f, "render worker is unresponsive"),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidConfiguration(_, source)
            | Self::InvalidBundle(_, source)
            | Self::InvalidResult(_, source)
            | Self::Snapshot(_, source)
            | Self::Canceled(source)
            | Self::WorkerStopped(source) => source.as_deref().map(|v| v as _),
            Self::InvalidJson(v) => Some(v),
            Self::WorkerStartup(v) => Some(v),
            Self::Cleanup { request, .. } => Some(request.as_ref()),
            _ => None,
        }
    }
}

/// The source of a poisoned lock. A poison error holds the guard, which is not `Send`, so the error keeps its message.
pub(crate) fn poisoned<T>(error: std::sync::PoisonError<T>) -> Source {
    Box::new(std::io::Error::other(error.to_string()))
}

/// The source of a panicked thread. The panic payload keeps its message, when it is a string.
pub(crate) fn panicked(payload: Box<dyn std::any::Any + Send>) -> Source {
    let message = match payload.downcast::<String>() {
        Ok(message) => *message,
        Err(payload) => match payload.downcast::<&'static str>() {
            Ok(message) => (*message).to_owned(),
            Err(_) => "the panic payload is not a string".to_owned(),
        },
    };
    Box::new(std::io::Error::other(format!("thread panicked: {message}")))
}
