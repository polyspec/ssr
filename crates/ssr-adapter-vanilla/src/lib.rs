#![forbid(unsafe_code)]

use ssr_core::{Page, Render, RenderResult, Value};
use ssr_runtime::{Pool, RenderMetrics};
use std::fmt;
use std::path::Path;

#[derive(Debug)]
pub enum Error {
    InvalidInput(&'static str),
    Runtime(ssr_runtime::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(value) => write!(f, "invalid vanilla adapter input: {value}"),
            Self::Runtime(value) => write!(f, "vanilla render failed: {value}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Runtime(value) => Some(value),
            Self::InvalidInput(_) => None,
        }
    }
}

impl From<ssr_runtime::Error> for Error {
    fn from(value: ssr_runtime::Error) -> Self {
        Self::Runtime(value)
    }
}

fn app_import(path: &str) -> Result<String, Error> {
    if !Path::new(path).is_absolute() {
        return Err(Error::InvalidInput("application path must be absolute"));
    }
    Ok(Value::string(path).compact())
}

pub fn server_entry(application: &str) -> Result<String, Error> {
    let application = app_import(application)?;
    Ok(format!(
        "import {{ render as renderApplication }} from {application};\nexport function render(props, state) {{ const renderState = {{input: state, output: null}}; const html = renderApplication(props, renderState); return {{html, head: '', state: renderState.output}}; }}\n"
    ))
}

pub fn client_entry(application: &str) -> Result<String, Error> {
    let application = app_import(application)?;
    Ok(format!(
        "import {{ hydrate, mount }} from {application};\nconst props = JSON.parse(document.getElementById('__SSR_PROPS__').textContent);\nconst renderState = {{input: JSON.parse(document.getElementById('__SSR_STATE__').textContent), output: null}};\nconst root = document.getElementById('root');\nif (root.dataset.render === 'ssr') hydrate(root, props, renderState); else if (root.dataset.render === 'csr') mount(root, props, renderState); else throw new Error('Invalid render mode');\n"
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

pub struct VanillaAdapter {
    client: String,
    styles: Vec<String>,
}

impl VanillaAdapter {
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

    pub fn render(&self, page: &Page, pool: &Pool) -> Result<RenderResult, Error> {
        self.render_with_metrics(page, pool)
            .map(|(result, _)| result)
    }

    pub fn render_with_metrics(
        &self,
        page: &Page,
        pool: &Pool,
    ) -> Result<(RenderResult, Option<RenderMetrics>), Error> {
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
        let html = self.document(page, &rendered.html, &rendered.head, &rendered.state)?;
        Ok((
            RenderResult {
                html,
                head: rendered.head,
                state: rendered.state,
            },
            metrics,
        ))
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
                "vanilla adapter cannot render head output",
            ));
        }
        if page.props.members().is_none() {
            return Err(Error::InvalidInput("props must be an object"));
        }
        let mut document = format!(
            "<!doctype html><html lang=\"{}\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>{}</title>",
            escape_html(&page.language),
            escape_html(&page.title),
        );
        for style in &self.styles {
            document.push_str(&format!("<link rel=\"stylesheet\" href=\"{style}\">"));
        }
        document.push_str(match page.render {
            Render::Ssr => "</head><body><div id=\"root\" data-render=\"ssr\">",
            Render::Csr => "</head><body><div id=\"root\" data-render=\"csr\">",
        });
        document.push_str(body);
        document.push_str("</div><script id=\"__SSR_PROPS__\" type=\"application/json\">");
        document.push_str(&script_json(&page.props));
        document.push_str("</script><script id=\"__SSR_STATE__\" type=\"application/json\">");
        document.push_str(&script_json(state));
        document.push_str(&format!(
            "</script><script type=\"module\" src=\"{}\"></script></body></html>",
            self.client
        ));
        Ok(document.into_bytes())
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
