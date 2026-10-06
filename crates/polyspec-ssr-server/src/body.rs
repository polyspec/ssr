use polyspec_ssr_nonce::NonceRewriter;
use polyspec_ssr_runtime::{Error as RuntimeError, Stream};
use std::fmt;

#[derive(Debug)]
pub enum BodyError {
    Runtime(RuntimeError),
    Html(String),
}

impl fmt::Display for BodyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Runtime(error) => write!(f, "stream rendering failed: {error}"),
            Self::Html(error) => write!(f, "HTML rewriting failed: {error}"),
        }
    }
}

impl std::error::Error for BodyError {}

impl From<polyspec_ssr_nonce::Error> for BodyError {
    fn from(error: polyspec_ssr_nonce::Error) -> Self {
        Self::Html(error.to_string())
    }
}

pub enum Body {
    Bytes(Option<Vec<u8>>),
    Stream(Box<StreamBody>),
}

impl Body {
    pub(crate) fn bytes(bytes: Vec<u8>) -> Self {
        Self::Bytes(Some(bytes))
    }

    pub(crate) fn html(bytes: Vec<u8>, nonce: &str) -> Result<Self, BodyError> {
        let mut rewriter = NonceRewriter::new(nonce);
        rewriter.write(&bytes)?;
        rewriter.end()?;
        let mut output = Vec::new();
        while let Some(chunk) = rewriter.pop() {
            output.extend_from_slice(&chunk);
        }
        Ok(Self::bytes(output))
    }

    pub(crate) fn stream(
        prefix: Vec<u8>,
        stream: Stream,
        suffix: Vec<u8>,
        nonce: &str,
    ) -> Result<Self, BodyError> {
        let mut rewriter = NonceRewriter::new(nonce);
        rewriter.write(&prefix)?;
        Ok(Self::Stream(Box::new(StreamBody {
            source: stream,
            suffix: Some(suffix),
            rewriter,
            finished: false,
        })))
    }

    pub fn collect_bytes(self) -> Result<Vec<u8>, BodyError> {
        self.collect::<Result<Vec<_>, _>>()
            .map(|chunks| chunks.concat())
    }
}

impl Iterator for Body {
    type Item = Result<Vec<u8>, BodyError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Bytes(bytes) => bytes.take().map(Ok),
            Self::Stream(stream) => stream.next(),
        }
    }
}

pub struct StreamBody {
    source: Stream,
    suffix: Option<Vec<u8>>,
    rewriter: NonceRewriter,
    finished: bool,
}

impl Iterator for StreamBody {
    type Item = Result<Vec<u8>, BodyError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(bytes) = self.rewriter.pop() {
                return Some(Ok(bytes));
            }
            if self.finished {
                return None;
            }
            match self.source.next() {
                Some(Ok(chunk)) => {
                    if let Err(error) = self.rewriter.write(&chunk) {
                        self.finished = true;
                        tracing::error!(%error, "stream body failed");
                        return Some(Err(error.into()));
                    }
                }
                Some(Err(error)) => {
                    self.finished = true;
                    tracing::error!(%error, "stream body failed");
                    return Some(Err(BodyError::Runtime(error)));
                }
                None => {
                    let suffix = self.suffix.take().expect("suffix present before end");
                    if let Err(error) = self
                        .rewriter
                        .write(&suffix)
                        .and_then(|_| self.rewriter.end())
                    {
                        self.finished = true;
                        tracing::error!(%error, "stream body failed");
                        return Some(Err(error.into()));
                    }
                    self.finished = true;
                }
            }
        }
    }
}

impl Drop for StreamBody {
    fn drop(&mut self) {
        if !self.finished {
            tracing::warn!("stream body was cancelled before completion");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Body;

    #[test]
    fn server_documents_apply_the_request_nonce_and_reject_mismatches() {
        let rewritten = Body::html(
            b"<html><body><script>x()</script></body></html>".to_vec(),
            "request",
        )
        .unwrap();
        let bytes = rewritten.collect_bytes().unwrap();
        assert!(bytes.ends_with(b"<script nonce=\"request\">x()</script></body></html>"));
        assert!(Body::html(b"<script nonce='other'>x()</script>".to_vec(), "request").is_err());
        assert!(Body::html(vec![0xff], "request").is_err());
    }
}
