use super::{DevelopmentError, process::Generation};
use hyper::body::{Body as HttpBody, Bytes, Frame, Incoming, SizeHint};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

pub(super) struct Body {
    incoming: Incoming,
    generation: Option<Arc<Generation>>,
}

impl Body {
    pub(super) fn new(incoming: Incoming, generation: Arc<Generation>) -> Self {
        Self {
            incoming,
            generation: Some(generation),
        }
    }
}

impl HttpBody for Body {
    type Data = Bytes;
    type Error = DevelopmentError;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, Self::Error>>> {
        match Pin::new(&mut self.incoming).poll_frame(cx) {
            Poll::Ready(None) => {
                self.generation.take();
                Poll::Ready(None)
            }
            Poll::Ready(Some(Err(error))) => {
                self.generation.take();
                Poll::Ready(Some(Err(DevelopmentError(format!(
                    "render response failed: {error:?}"
                )))))
            }
            Poll::Ready(Some(Ok(frame))) => Poll::Ready(Some(Ok(frame))),
            Poll::Pending => Poll::Pending,
        }
    }

    fn is_end_stream(&self) -> bool {
        self.incoming.is_end_stream()
    }
    fn size_hint(&self) -> SizeHint {
        self.incoming.size_hint()
    }
}
