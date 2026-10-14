//! The request-nonce document rewriter of the SSR server contract.
//!
//! Every inline `script` and `style` element of one rendered document uses
//! the nonce of its request. An element that already has a different nonce
//! is an error; the rewriter never silently replaces one.

use lol_html::{HtmlRewriter, Settings, element, send::SendHandlerTypes};
use std::{
    collections::VecDeque,
    fmt,
    sync::{Arc, Mutex},
};

type Sink = Box<dyn FnMut(&[u8]) + Send>;
type Rewriter = HtmlRewriter<'static, Sink, SendHandlerTypes>;

#[derive(Debug)]
pub enum Error {
    Html(String),
    Utf8(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Html(error) => write!(f, "HTML rewriting failed: {error}"),
            Self::Utf8(error) => write!(f, "rendered HTML is not UTF-8: {error}"),
        }
    }
}

impl std::error::Error for Error {}

/// Applies the request nonce to every `script` and `style` element of the
/// document chunks written into it.
pub struct NonceRewriter {
    rewriter: Option<Rewriter>,
    output: Arc<Mutex<VecDeque<Vec<u8>>>>,
    pending_utf8: Vec<u8>,
}

impl NonceRewriter {
    pub fn new(nonce: &str) -> Self {
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

    pub fn write(&mut self, chunk: &[u8]) -> Result<(), Error> {
        self.pending_utf8.extend_from_slice(chunk);
        match std::str::from_utf8(&self.pending_utf8) {
            Ok(_) => self.pending_utf8.clear(),
            Err(error) if error.error_len().is_none() => {
                let valid = error.valid_up_to();
                self.pending_utf8.drain(..valid);
            }
            Err(error) => return Err(Error::Utf8(error.to_string())),
        }
        self.rewriter
            .as_mut()
            .expect("rewriter active before end")
            .write(chunk)
            .map_err(|error| Error::Html(error.to_string()))
    }

    pub fn end(&mut self) -> Result<(), Error> {
        if !self.pending_utf8.is_empty() {
            return Err(Error::Utf8(
                "incomplete UTF-8 sequence at document end".into(),
            ));
        }
        self.rewriter
            .take()
            .expect("rewriter active before end")
            .end()
            .map_err(|error| Error::Html(error.to_string()))
    }

    pub fn pop(&self) -> Option<Vec<u8>> {
        self.output
            .lock()
            .expect("nonce output lock poisoned")
            .pop_front()
    }
}

#[cfg(test)]
mod tests {
    use super::NonceRewriter;

    fn rewritten(chunks: &[&[u8]], nonce: &str) -> String {
        let mut rewriter = NonceRewriter::new(nonce);
        for chunk in chunks {
            rewriter.write(chunk).unwrap();
        }
        rewriter.end().unwrap();
        let mut result = Vec::new();
        while let Some(chunk) = rewriter.pop() {
            result.extend_from_slice(&chunk);
        }
        String::from_utf8(result).unwrap()
    }

    #[test]
    fn rewriter_handles_split_tags_raw_text_and_comments() {
        let result = rewritten(
            &[
                b"<!doctype html><html><head><!-- <script>fake</script> --><ST".as_slice(),
                b"YLE data-x='a>b'>.note::after{content:'<script>fake</script>'}</STYLE>"
                    .as_slice(),
                b"</head><body><script data-x=\"a>b\">const text = '<style>fake</style>';"
                    .as_slice(),
                b"</script><script nonce=\"request_nonce\">ok()</script></body></html>".as_slice(),
            ],
            "request_nonce",
        );
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
    fn mismatched_inline_nonce_is_an_error() {
        fn assert_document_error(document: &[u8], nonce: &str) {
            let mut rewriter = NonceRewriter::new(nonce);
            if rewriter.write(document).is_ok() {
                assert!(rewriter.end().is_err());
            }
        }
        assert_document_error(b"<script nonce='other'>x()</script>", "request");
        assert_document_error(b"<style nonce=other>x</style>", "request");
    }

    #[test]
    fn invalid_utf8_is_an_error() {
        let mut rewriter = NonceRewriter::new("request");
        assert!(rewriter.write(&[0xff]).is_err());
        let mut rewriter = NonceRewriter::new("request");
        rewriter.write(&[0xc3]).unwrap();
        assert!(rewriter.end().is_err());
    }
}
