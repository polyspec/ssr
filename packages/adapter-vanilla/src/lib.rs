use polyspec_ssr_adapter_support::{Assets, HeadOutput, app_import};
use polyspec_ssr_core::{Page, RenderResult};
use polyspec_ssr_runtime::{Pool, RenderMetrics};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid vanilla adapter input: {0}")]
    InvalidInput(&'static str),
    #[error("vanilla render failed: {0}")]
    Runtime(#[from] polyspec_ssr_runtime::Error),
}

pub fn server_entry(application: &str) -> Result<String, Error> {
    let application = app_import(application).map_err(Error::InvalidInput)?;
    Ok(format!(
        "import {{ render as renderApplication }} from {application};\nexport function render(props, state) {{ const renderState = {{input: state, output: null}}; const html = renderApplication(props, renderState); return {{html, head: '', state: renderState.output}}; }}\n"
    ))
}

pub fn client_entry(application: &str) -> Result<String, Error> {
    let application = app_import(application).map_err(Error::InvalidInput)?;
    Ok(format!(
        "import {{ hydrate, mount }} from {application};\nconst props = JSON.parse(document.getElementById('__SSR_PROPS__').textContent);\nconst renderState = {{input: JSON.parse(document.getElementById('__SSR_STATE__').textContent), output: null}};\nconst root = document.getElementById('root');\nif (root.dataset.render === 'ssr') hydrate(root, props, renderState); else if (root.dataset.render === 'csr') mount(root, props, renderState); else throw new Error('Invalid render mode');\n"
    ))
}

pub struct VanillaAdapter {
    assets: Assets,
}

impl VanillaAdapter {
    pub fn new(client: &str, styles: &[&str]) -> Result<Self, Error> {
        Assets::new(
            client,
            styles,
            HeadOutput::Rejected("vanilla adapter cannot render head output"),
        )
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
