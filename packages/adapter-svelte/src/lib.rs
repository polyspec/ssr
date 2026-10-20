use polyspec_ssr_adapter_support::{Assets, HeadOutput, entry};
use std::ffi::OsStr;
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid Svelte adapter input: {0}")]
    InvalidInput(&'static str),
    #[error("Svelte render failed: {0}")]
    Runtime(#[from] polyspec_ssr_runtime::Error),
}

fn check_application(path: &str) -> Result<(), Error> {
    if !Path::new(path).is_absolute() || Path::new(path).extension() != Some(OsStr::new("svelte")) {
        return Err(Error::InvalidInput(
            "application path must be an absolute .svelte file",
        ));
    }
    Ok(())
}

pub fn server_entry(application: &str) -> Result<String, Error> {
    check_application(application)?;
    entry(application, "import * as svelteServer from 'svelte/server';\nimport App from {application};\nexport function render(props, state) { const renderState = {input: state, output: null}; const result = svelteServer.render(App, {props: {...props, renderState}}); return {html: result.body, head: result.head, state: renderState.output}; }\n").map_err(Error::InvalidInput)
}

pub fn client_entry(application: &str) -> Result<String, Error> {
    check_application(application)?;
    entry(application, "import { hydrate, mount } from 'svelte';\nimport App from {application};\nconst props = JSON.parse(document.getElementById('__SSR_PROPS__').textContent);\nconst renderState = {input: JSON.parse(document.getElementById('__SSR_STATE__').textContent), output: null};\nconst root = document.getElementById('root');\nif (root.dataset.render === 'ssr') hydrate(App, {target: root, props: {...props, renderState}, recover: false}); else if (root.dataset.render === 'csr') mount(App, {target: root, props: {...props, renderState}}); else throw new Error('Invalid render mode');\n").map_err(Error::InvalidInput)
}

pub struct SvelteAdapter {
    assets: Assets,
}

polyspec_ssr_adapter_support::adapter!(SvelteAdapter, Error, HeadOutput::Inserted, with_metrics);
