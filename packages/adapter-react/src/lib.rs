use polyspec_ssr_adapter_support::{Assets, HeadOutput, entry};
use polyspec_ssr_core::{Page, Render, RenderResult, Value};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid React adapter input: {0}")]
    InvalidInput(#[source] polyspec_ssr_adapter_support::Error),
}

pub fn server_entry(application: &str) -> Result<String, Error> {
    entry(
        application,
        "import React from 'react';\nimport App from {application};\nglobalThis.__ssrApp = App;\n",
    )
    .map_err(Error::InvalidInput)
}

pub fn framework_entry() -> &'static str {
    "import React from 'react';\nimport * as JSX from 'react/jsx-runtime';\nimport {renderToReadableStream} from 'react-dom/server.edge';\nimport {ReadableStream as SSRReadableStream} from 'web-streams-polyfill';\nReadableStream = SSRReadableStream;\nglobalThis.__ssrReact = React;\nglobalThis.__ssrJsxRuntime = JSX;\nglobalThis.render = async (App, props, state, nonce) => { let output = null; let fixed = false; const renderState = { input: state, get output() { return output; }, set output(value) { if (fixed) throw new Error('render state changed after shell'); output = value; } }; const stream = await renderToReadableStream(React.createElement(App, {...props, renderState}), { nonce, onError: globalThis.__ssrReportReactError }); fixed = true; return {stream, state: output}; };\n"
}

pub fn client_entry(application: &str) -> Result<String, Error> {
    entry(
        application,
        "import React from 'react';\nimport {createRoot, hydrateRoot} from 'react-dom/client';\nimport App from {application};\nconst props = JSON.parse(document.getElementById('__SSR_PROPS__').textContent);\nconst renderState = {input: JSON.parse(document.getElementById('__SSR_STATE__').textContent), output: null};\nconst root = document.getElementById('root');\nconst app = <App {...props} renderState={renderState} />;\nif (root.dataset.render === 'ssr') hydrateRoot(root, app); else if (root.dataset.render === 'csr') createRoot(root).render(app); else throw new Error('Invalid render mode');\n",
    )
    .map_err(Error::InvalidInput)
}

pub struct ReactAdapter {
    assets: Assets,
}

impl ReactAdapter {
    pub fn new(client: &str, styles: &[&str]) -> Result<Self, Error> {
        Assets::new(
            client,
            styles,
            HeadOutput::Rejected("React adapter cannot render head output"),
        )
        .map(|assets| Self { assets })
        .map_err(Error::InvalidInput)
    }

    pub fn render(&self, page: &Page) -> Result<RenderResult, Error> {
        if page.render != Render::Csr {
            return Err(Error::InvalidInput("SSR requires stream rendering".into()));
        }
        let html = self
            .assets
            .document(page, &[], &[], &page.state)
            .map_err(Error::InvalidInput)?;
        Ok(RenderResult {
            html,
            head: Vec::new(),
            state: page.state.clone(),
        })
    }

    pub fn static_shell(&self, page: &Page) -> Result<Vec<u8>, Error> {
        self.assets.static_shell(page).map_err(Error::InvalidInput)
    }

    pub fn stream_parts(&self, page: &Page, state: &Value) -> Result<(Vec<u8>, Vec<u8>), Error> {
        self.assets
            .parts(page, state, "")
            .map_err(Error::InvalidInput)
    }
}
