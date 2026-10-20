use polyspec_ssr_adapter_support::{Assets, HeadOutput, entry};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid Vue adapter input: {0}")]
    InvalidInput(&'static str),
    #[error("Vue render failed: {0}")]
    Runtime(#[from] polyspec_ssr_runtime::Error),
}

pub fn server_entry(application: &str) -> Result<String, Error> {
    entry(
        application,
        "import { createSSRApp } from 'vue';\nimport { renderToString } from 'vue/server-renderer';\nimport App from {application};\nexport async function render(props, state) { const renderState = {input: state, output: null}; const html = await renderToString(createSSRApp(App, {...props, renderState})); return {html, head: '', state: renderState.output}; }\n",
    )
    .map_err(Error::InvalidInput)
}

pub fn client_entry(application: &str) -> Result<String, Error> {
    entry(
        application,
        "import { createApp, createSSRApp } from 'vue';\nimport App from {application};\nconst props = JSON.parse(document.getElementById('__SSR_PROPS__').textContent);\nconst renderState = {input: JSON.parse(document.getElementById('__SSR_STATE__').textContent), output: null};\nconst root = document.getElementById('root');\nconst app = root.dataset.render === 'ssr' ? createSSRApp(App, {...props, renderState}) : root.dataset.render === 'csr' ? createApp(App, {...props, renderState}) : null;\nif (app === null) throw new Error('Invalid render mode');\napp.mount(root);\n",
    )
    .map_err(Error::InvalidInput)
}

pub struct VueAdapter {
    assets: Assets,
}

polyspec_ssr_adapter_support::adapter!(
    VueAdapter,
    Error,
    HeadOutput::Rejected("Vue adapter cannot render head output")
);
