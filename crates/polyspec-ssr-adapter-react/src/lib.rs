#![forbid(unsafe_code)]

use polyspec_ssr_core::{Page, Render, RenderResult, Value};
use std::fmt;
use std::path::Path;

#[derive(Debug)]
pub enum Error {
    InvalidInput(&'static str),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(value) => write!(f, "invalid React adapter input: {value}"),
        }
    }
}

impl std::error::Error for Error {}

fn app_import(path: &str) -> Result<String, Error> {
    if !Path::new(path).is_absolute() {
        return Err(Error::InvalidInput("application path must be absolute"));
    }
    Ok(Value::string(path).compact())
}

pub fn server_entry(application: &str) -> Result<String, Error> {
    let application = app_import(application)?;
    Ok(format!(
        "import React from 'react';\nimport App from {application};\nglobalThis.__ssrApp = App;\n"
    ))
}

pub fn framework_entry() -> &'static str {
    "import React from 'react';\nimport * as JSX from 'react/jsx-runtime';\nimport {renderToReadableStream} from 'react-dom/server.edge';\nimport {ReadableStream as SSRReadableStream} from 'web-streams-polyfill';\nReadableStream = SSRReadableStream;\nglobalThis.__ssrReact = React;\nglobalThis.__ssrJsxRuntime = JSX;\nglobalThis.render = async (App, props, state, nonce) => { let output = null; let fixed = false; const renderState = { input: state, get output() { return output; }, set output(value) { if (fixed) throw new Error('render state changed after shell'); output = value; } }; const stream = await renderToReadableStream(React.createElement(App, {...props, renderState}), { nonce, onError: globalThis.__ssrReportReactError }); fixed = true; return {stream, state: output}; };\n"
}

pub fn client_entry(application: &str) -> Result<String, Error> {
    let application = app_import(application)?;
    Ok(format!(
        "import React from 'react';\nimport {{createRoot, hydrateRoot}} from 'react-dom/client';\nimport App from {application};\nconst props = JSON.parse(document.getElementById('__SSR_PROPS__').textContent);\nconst renderState = {{input: JSON.parse(document.getElementById('__SSR_STATE__').textContent), output: null}};\nconst root = document.getElementById('root');\nconst app = <App {{...props}} renderState={{renderState}} />;\nif (root.dataset.render === 'ssr') hydrateRoot(root, app); else if (root.dataset.render === 'csr') createRoot(root).render(app); else throw new Error('Invalid render mode');\n"
    ))
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

pub struct ReactAdapter {
    client: String,
    styles: Vec<String>,
}

impl ReactAdapter {
    pub fn new(client: &str, styles: &[&str]) -> Result<Self, Error> {
        if !local_url(client) || !client.ends_with(".js") {
            return Err(Error::InvalidInput(
                "client URL must be a local JavaScript path",
            ));
        }
        if styles
            .iter()
            .any(|style| !local_url(style) || !style.ends_with(".css"))
        {
            return Err(Error::InvalidInput("style URL must be a local CSS path"));
        }
        Ok(Self {
            client: client.into(),
            styles: styles.iter().map(|style| (*style).into()).collect(),
        })
    }

    pub fn render(&self, page: &Page) -> Result<RenderResult, Error> {
        if page.render != Render::Csr {
            return Err(Error::InvalidInput("SSR requires stream rendering"));
        }
        let html = self.document(page, &[], &[], &page.state)?;
        Ok(RenderResult {
            html,
            head: Vec::new(),
            state: page.state.clone(),
        })
    }

    pub fn static_shell(&self, page: &Page) -> Result<Vec<u8>, Error> {
        if page.render != Render::Csr
            || page.props.members().is_none_or(|props| !props.is_empty())
            || page.state.compact() != "null"
        {
            return Err(Error::InvalidInput(
                "static shell requires CSR, empty props and null state",
            ));
        }
        self.document(page, &[], &[], &page.state)
    }

    fn document(
        &self,
        page: &Page,
        body: &[u8],
        head: &[u8],
        state: &Value,
    ) -> Result<Vec<u8>, Error> {
        let body = std::str::from_utf8(body)
            .map_err(|_| Error::InvalidInput("rendered HTML must be UTF-8"))?;
        let head = std::str::from_utf8(head)
            .map_err(|_| Error::InvalidInput("rendered head must be UTF-8"))?;
        if !head.is_empty() {
            return Err(Error::InvalidInput(
                "React adapter cannot render head output",
            ));
        }
        let (mut prefix, suffix) = self.stream_parts(page, state)?;
        prefix.extend_from_slice(body.as_bytes());
        prefix.extend_from_slice(&suffix);
        Ok(prefix)
    }

    pub fn stream_parts(&self, page: &Page, state: &Value) -> Result<(Vec<u8>, Vec<u8>), Error> {
        if page.props.members().is_none() {
            return Err(Error::InvalidInput("props must be an object"));
        }
        let mut prefix = format!(
            "<!doctype html><html lang=\"{}\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>{}</title>",
            escape_html(&page.language),
            escape_html(&page.title),
        );
        for style in &self.styles {
            prefix.push_str(&format!("<link rel=\"stylesheet\" href=\"{style}\">"));
        }
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
