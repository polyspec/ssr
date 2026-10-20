//! The code that the framework adapters share: the page document around rendered HTML, the validation of the client and
//! style URLs and of the application path, and the render of a page through a pool. Only the adapter crates of this
//! repository use it; every error is the message of the adapter's `InvalidInput`.

use polyspec_ssr_core::{Page, Render, RenderResult, Value};
use polyspec_ssr_runtime::{Pool, RenderMetrics};
use std::path::Path;

/// The JSON string of an application path, the module specifier of a generated entry.
pub fn import_specifier(path: &str) -> String {
    Value::string(path).compact()
}

/// The module specifier of an absolute application path.
pub fn app_import(path: &str) -> Result<String, &'static str> {
    if !Path::new(path).is_absolute() {
        return Err("application path must be absolute");
    }
    Ok(import_specifier(path))
}

fn local_url(value: &str) -> bool {
    value.starts_with('/')
        && !value.starts_with("//")
        && value
            .split('/')
            .skip(1)
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'_' | b'.'))
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn script_json(value: &Value) -> String {
    value
        .compact()
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('&', "\\u0026")
}

/// What an adapter does with the head that the render returns: rejects it with its message, or inserts it after the
/// style links.
#[derive(Clone, Copy)]
pub enum HeadOutput {
    Rejected(&'static str),
    Inserted,
}

/// The client script and the style sheets that a page document links.
pub struct Assets {
    client: String,
    styles: Vec<String>,
    head: HeadOutput,
}

impl Assets {
    /// The assets of local URLs: a JavaScript client and CSS styles.
    pub fn new(client: &str, styles: &[&str], head: HeadOutput) -> Result<Self, &'static str> {
        if !local_url(client) || !client.ends_with(".js") {
            return Err("client URL must be a local JavaScript path");
        }
        if styles
            .iter()
            .any(|style| !local_url(style) || !style.ends_with(".css"))
        {
            return Err("style URL must be a local CSS path");
        }
        Ok(Self {
            client: client.into(),
            styles: styles.iter().map(|style| (*style).into()).collect(),
            head,
        })
    }

    /// The document before and after the rendered body, with `head` after the style links.
    pub fn parts(
        &self,
        page: &Page,
        state: &Value,
        head: &str,
    ) -> Result<(Vec<u8>, Vec<u8>), &'static str> {
        if page.props.members().is_none() {
            return Err("props must be an object");
        }
        let mut prefix = format!(
            "<!doctype html><html lang=\"{}\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>{}</title>",
            escape_html(&page.language),
            escape_html(&page.title),
        );
        for style in &self.styles {
            prefix.push_str(&format!("<link rel=\"stylesheet\" href=\"{style}\">"));
        }
        prefix.push_str(head);
        prefix.push_str(match page.render {
            Render::Ssr => "</head><body><div id=\"root\" data-render=\"ssr\">",
            Render::Csr => "</head><body><div id=\"root\" data-render=\"csr\">",
        });
        let suffix = format!(
            "</div><script id=\"__SSR_PROPS__\" type=\"application/json\">{}</script><script id=\"__SSR_STATE__\" type=\"application/json\">{}</script><script type=\"module\" src=\"{}\"></script></body></html>",
            script_json(&page.props),
            script_json(state),
            self.client
        );
        Ok((prefix.into_bytes(), suffix.into_bytes()))
    }

    /// The whole document around the rendered `body` and `head`, which must be UTF-8.
    pub fn document(
        &self,
        page: &Page,
        body: &[u8],
        head: &[u8],
        state: &Value,
    ) -> Result<Vec<u8>, &'static str> {
        let body = std::str::from_utf8(body).map_err(|_| "rendered HTML must be UTF-8")?;
        let head = std::str::from_utf8(head).map_err(|_| "rendered head must be UTF-8")?;
        if let HeadOutput::Rejected(message) = self.head
            && !head.is_empty()
        {
            return Err(message);
        }
        let (mut document, suffix) = self.parts(page, state, head)?;
        document.extend_from_slice(body.as_bytes());
        document.extend_from_slice(&suffix);
        Ok(document)
    }

    /// The document of a static shell: client rendering with empty props and a null state.
    pub fn static_shell(&self, page: &Page) -> Result<Vec<u8>, &'static str> {
        if page.render != Render::Csr
            || page.props.members().is_none_or(|props| !props.is_empty())
            || page.state.compact() != "null"
        {
            return Err("static shell requires CSR, empty props and null state");
        }
        self.document(page, &[], &[], &page.state)
    }

    /// Renders `page` and makes its result with the document around the rendered result: the pool renders server
    /// rendering and its metrics are returned, and client rendering has an empty body and no metrics. `invalid` makes the
    /// error of an input that the document refuses.
    pub fn render<E: From<polyspec_ssr_runtime::Error>>(
        &self,
        page: &Page,
        pool: &Pool,
        invalid: fn(&'static str) -> E,
    ) -> Result<(RenderResult, Option<RenderMetrics>), E> {
        let (rendered, metrics) = match page.render {
            Render::Ssr => {
                let (rendered, metrics) = pool.render_with_metrics(page)?;
                (rendered, Some(metrics))
            }
            Render::Csr => (
                RenderResult {
                    html: Vec::new(),
                    head: Vec::new(),
                    state: page.state.clone(),
                },
                None,
            ),
        };
        let html = self
            .document(page, &rendered.html, &rendered.head, &rendered.state)
            .map_err(invalid)?;
        Ok((
            RenderResult {
                html,
                head: rendered.head,
                state: rendered.state,
            },
            metrics,
        ))
    }
}
