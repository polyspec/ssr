use polyspec_ssr_adapter_support::{Assets, HeadOutput, import_specifier};
use polyspec_ssr_core::{Page, RenderResult};
use polyspec_ssr_runtime::{Pool, RenderMetrics};
use std::ffi::OsStr;
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid Svelte adapter input: {0}")]
    InvalidInput(&'static str),
    #[error("Svelte render failed: {0}")]
    Runtime(#[from] polyspec_ssr_runtime::Error),
}

fn app_import(path: &str) -> Result<String, Error> {
    if !Path::new(path).is_absolute() || Path::new(path).extension() != Some(OsStr::new("svelte")) {
        return Err(Error::InvalidInput(
            "application path must be an absolute .svelte file",
        ));
    }
    Ok(import_specifier(path))
}

pub fn server_entry(application: &str) -> Result<String, Error> {
    let application = app_import(application)?;
    Ok(format!(
        "import * as svelteServer from 'svelte/server';\nimport App from {application};\nexport function render(props, state) {{ const renderState = {{input: state, output: null}}; const result = svelteServer.render(App, {{props: {{...props, renderState}}}}); return {{html: result.body, head: result.head, state: renderState.output}}; }}\n"
    ))
}

pub fn client_entry(application: &str) -> Result<String, Error> {
    let application = app_import(application)?;
    Ok(format!(
        "import {{ hydrate, mount }} from 'svelte';\nimport App from {application};\nconst props = JSON.parse(document.getElementById('__SSR_PROPS__').textContent);\nconst renderState = {{input: JSON.parse(document.getElementById('__SSR_STATE__').textContent), output: null}};\nconst root = document.getElementById('root');\nif (root.dataset.render === 'ssr') hydrate(App, {{target: root, props: {{...props, renderState}}, recover: false}}); else if (root.dataset.render === 'csr') mount(App, {{target: root, props: {{...props, renderState}}}}); else throw new Error('Invalid render mode');\n"
    ))
}

pub struct SvelteAdapter {
    assets: Assets,
}

impl SvelteAdapter {
    pub fn new(client: &str, styles: &[&str]) -> Result<Self, Error> {
        Assets::new(client, styles, HeadOutput::Inserted)
            .map(|assets| Self { assets })
            .map_err(Error::InvalidInput)
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
        self.assets.render(page, pool, Error::InvalidInput)
    }

    pub fn static_shell(&self, page: &Page) -> Result<Vec<u8>, Error> {
        self.assets.static_shell(page).map_err(Error::InvalidInput)
    }
}
