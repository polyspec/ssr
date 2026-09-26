use lol_html::{HtmlRewriter, Settings, element, send::SendHandlerTypes};
use ssr_runtime::{Error as RuntimeError, Stream};
use std::collections::VecDeque;
use std::fmt;
use std::sync::{Arc, Mutex};

type Sink = Box<dyn FnMut(&[u8]) + Send>;
type Rewriter = HtmlRewriter<'static, Sink, SendHandlerTypes>;

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

struct NonceRewriter {
    rewriter: Option<Rewriter>,
    output: Arc<Mutex<VecDeque<Vec<u8>>>>,
    pending_utf8: Vec<u8>,
}

impl NonceRewriter {
    fn new(nonce: &str) -> Self {
        let nonce = nonce.to_owned();
        let settings = Settings::new_send().append_element_content_handler(element!(
            "script, style",
            move |element: &mut lol_html::send::Element<'_, '_>| {
                if let Some(existing) = element.get_attribute("nonce") {
                    if existing != nonce {
                        return Err("inline nonce differs from the request nonce".into());
                    }
                } else {
                    element.set_attribute("nonce", &nonce)?;
                }
                Ok(())
            }
        ));
        let output = Arc::new(Mutex::new(VecDeque::new()));
        let sink_output = Arc::clone(&output);
        let sink: Sink = Box::new(move |chunk| {
            if !chunk.is_empty() {
                sink_output
                    .lock()
                    .expect("nonce output lock poisoned")
                    .push_back(chunk.to_vec());
            }
        });
        Self {
            rewriter: Some(HtmlRewriter::new(settings, sink)),
            output,
            pending_utf8: Vec::new(),
        }
    }

    fn write(&mut self, chunk: &[u8]) -> Result<(), BodyError> {
        self.pending_utf8.extend_from_slice(chunk);
        match std::str::from_utf8(&self.pending_utf8) {
            Ok(_) => self.pending_utf8.clear(),
            Err(error) if error.error_len().is_none() => {
                let valid = error.valid_up_to();
                self.pending_utf8.drain(..valid);
            }
            Err(error) => return Err(BodyError::Html(format!("invalid UTF-8: {error}"))),
        }
        self.rewriter
            .as_mut()
            .expect("rewriter active before end")
            .write(chunk)
            .map_err(|error| BodyError::Html(error.to_string()))
    }

    fn end(&mut self) -> Result<(), BodyError> {
        if !self.pending_utf8.is_empty() {
            return Err(BodyError::Html("incomplete UTF-8 at document end".into()));
        }
        self.rewriter
            .take()
            .expect("rewriter active before end")
            .end()
            .map_err(|error| BodyError::Html(error.to_string()))
    }

    fn pop(&self) -> Option<Vec<u8>> {
        self.output
            .lock()
            .expect("nonce output lock poisoned")
            .pop_front()
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
                        return Some(Err(error));
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
                        return Some(Err(error));
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
    use super::{Body, NonceRewriter};

    #[test]
    fn nonce_rewriter_handles_split_tags_raw_text_and_comments() {
        let mut rewriter = NonceRewriter::new("request_nonce");
        for chunk in [
            b"<!doctype html><html><head><!-- <script>fake</script> --><ST".as_slice(),
            b"YLE data-x='a>b'>.note::after{content:'<script>fake</script>'}</STYLE>".as_slice(),
            b"</head><body><script data-x=\"a>b\">const text = '<style>fake</style>';".as_slice(),
            b"</script><script nonce=\"request_nonce\">ok()</script></body></html>".as_slice(),
        ] {
            rewriter.write(chunk).unwrap();
        }
        rewriter.end().unwrap();
        let mut result = Vec::new();
        while let Some(chunk) = rewriter.pop() {
            result.extend_from_slice(&chunk);
        }
        let result = String::from_utf8(result).unwrap();
        assert!(result.contains("<!-- <script>fake</script> -->"));
        assert!(
            result.contains("<STYLE data-x='a>b' nonce=\"request_nonce\">"),
            "{result}"
        );
        assert!(result.contains("content:'<script>fake</script>'"));
        assert!(
            result.contains("<script data-x=\"a>b\" nonce=\"request_nonce\">"),
            "{result}"
        );
        assert!(result.contains("const text = '<style>fake</style>';"));
        assert!(result.contains("<script nonce=\"request_nonce\">ok()"));
    }

    #[test]
    fn nonce_mismatch_and_invalid_utf8_are_errors() {
        assert!(Body::html(b"<script nonce='other'>x()</script>".to_vec(), "request").is_err());
        assert!(Body::html(b"<style nonce=other>x</style>".to_vec(), "request").is_err());
        assert!(Body::html(vec![0xff], "request").is_err());
        let mut rewriter = NonceRewriter::new("request");
        rewriter.write(&[0xc3]).unwrap();
        assert!(rewriter.end().is_err());
    }
}
