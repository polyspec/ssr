use polyspec_ssr_adapter_support::{Assets, HeadOutput, app_import};
use polyspec_ssr_core::{Page, RenderResult};
use polyspec_ssr_runtime::Pool;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid Vue adapter input: {0}")]
    InvalidInput(&'static str),
    #[error("Vue render failed: {0}")]
    Runtime(#[from] polyspec_ssr_runtime::Error),
}

pub fn server_entry(application: &str) -> Result<String, Error> {
    let application = app_import(application).map_err(Error::InvalidInput)?;
    Ok(format!(
        "import {{ createSSRApp }} from 'vue';\nimport {{ renderToString }} from 'vue/server-renderer';\nimport App from {application};\nexport async function render(props, state) {{ const renderState = {{input: state, output: null}}; const html = await renderToString(createSSRApp(App, {{...props, renderState}})); return {{html, head: '', state: renderState.output}}; }}\n"
    ))
}

pub fn client_entry(application: &str) -> Result<String, Error> {
    let application = app_import(application).map_err(Error::InvalidInput)?;
    Ok(format!(
        "import {{ createApp, createSSRApp }} from 'vue';\nimport App from {application};\nconst props = JSON.parse(document.getElementById('__SSR_PROPS__').textContent);\nconst renderState = {{input: JSON.parse(document.getElementById('__SSR_STATE__').textContent), output: null}};\nconst root = document.getElementById('root');\nconst app = root.dataset.render === 'ssr' ? createSSRApp(App, {{...props, renderState}}) : root.dataset.render === 'csr' ? createApp(App, {{...props, renderState}}) : null;\nif (app === null) throw new Error('Invalid render mode');\napp.mount(root);\n"
    ))
}

pub struct VueAdapter {
    assets: Assets,
}

impl VueAdapter {
    pub fn new(client: &str, styles: &[&str]) -> Result<Self, Error> {
        Assets::new(
            client,
            styles,
            HeadOutput::Rejected("Vue adapter cannot render head output"),
        )
        .map(|assets| Self { assets })
        .map_err(Error::InvalidInput)
    }

    pub fn render(&self, page: &Page, pool: &Pool) -> Result<RenderResult, Error> {
        self.assets
            .render(page, pool, Error::InvalidInput)
            .map(|(result, _)| result)
    }

    pub fn static_shell(&self, page: &Page) -> Result<Vec<u8>, Error> {
        self.assets.static_shell(page).map_err(Error::InvalidInput)
    }
}
