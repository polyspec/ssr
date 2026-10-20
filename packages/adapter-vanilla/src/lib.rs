use polyspec_ssr_adapter_support::{Assets, HeadOutput, entry};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid vanilla adapter input: {0}")]
    InvalidInput(#[source] polyspec_ssr_adapter_support::Error),
    #[error("vanilla render failed: {0}")]
    Runtime(#[from] polyspec_ssr_runtime::Error),
}

pub fn server_entry(application: &str) -> Result<String, Error> {
    entry(
        application,
        "import { render as renderApplication } from {application};\nexport function render(props, state) { const renderState = {input: state, output: null}; const html = renderApplication(props, renderState); return {html, head: '', state: renderState.output}; }\n",
    )
    .map_err(Error::InvalidInput)
}

pub fn client_entry(application: &str) -> Result<String, Error> {
    entry(
        application,
        "import { hydrate, mount } from {application};\nconst props = JSON.parse(document.getElementById('__SSR_PROPS__').textContent);\nconst renderState = {input: JSON.parse(document.getElementById('__SSR_STATE__').textContent), output: null};\nconst root = document.getElementById('root');\nif (root.dataset.render === 'ssr') hydrate(root, props, renderState); else if (root.dataset.render === 'csr') mount(root, props, renderState); else throw new Error('Invalid render mode');\n",
    )
    .map_err(Error::InvalidInput)
}

pub struct VanillaAdapter {
    assets: Assets,
}

polyspec_ssr_adapter_support::adapter!(
    VanillaAdapter,
    Error,
    HeadOutput::Rejected("vanilla adapter cannot render head output"),
    with_metrics
);
