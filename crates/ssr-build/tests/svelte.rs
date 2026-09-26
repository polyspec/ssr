use ssr_build::{BuildConfig, build};
use std::fs;
use std::path::Path;

#[tokio::test]
async fn svelte_source_compiles_in_rust_and_publishes_component_css() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/build-probe/tests/fixtures")
        .canonicalize()
        .unwrap();
    assert!(root.join("node_modules/svelte/compiler/index.js").is_file());
    let generated = root.join("node_modules/.ssr-build-svelte");
    fs::create_dir_all(&generated).unwrap();
    let application = root.join("SvelteApp.svelte");
    let application = ordered_json::Value::string(application.to_str().unwrap()).compact();
    let server = generated.join("server.js");
    let client = generated.join("client.js");
    fs::write(&server, format!("import * as svelteServer from 'svelte/server'; import App from {application}; export function render(props, state) {{ const renderState = {{input: state, output: null}}; const result = svelteServer.render(App, {{props: {{...props, renderState}}}}); return {{html: result.body, head: result.head, state: renderState.output}}; }}" )).unwrap();
    fs::write(&client, format!("import {{hydrate, mount}} from 'svelte'; import App from {application}; const props = JSON.parse(document.getElementById('__SSR_PROPS__').textContent); const renderState = {{input: JSON.parse(document.getElementById('__SSR_STATE__').textContent), output: null}}; const root = document.getElementById('root'); if (root.dataset.render === 'ssr') hydrate(App, {{target: root, props: {{...props, renderState}}, recover: false}}); else if (root.dataset.render === 'csr') mount(App, {{target: root, props: {{...props, renderState}}}}); else throw new Error('Invalid render mode');" )).unwrap();
    let config = BuildConfig {
        root: root.clone(),
        server_entry: server,
        client_entry: client,
        css_entry: root.join("app.css"),
        asset_route: "/assets".into(),
        react_framework_entry: None,
    };
    let output = build(&config).await.unwrap();
    assert!(output.manifest.styles.len() >= 2);
    assert!(output.manifest.styles.iter().any(|style| {
        output.files[&style.path]
            .windows(b"#123456".len())
            .any(|part| part == b"#123456")
    }));
    assert!(output.files[&output.manifest.server.path].len() > 100);
    assert!(output.files[&output.manifest.client.path].len() > 100);
    let mut has_svelte_source = false;
    for javascript in
        std::iter::once(&output.manifest.server).chain(output.manifest.server_chunks.iter())
    {
        let map_path = format!("{}.map", javascript.path);
        let map = output
            .manifest
            .source_maps
            .iter()
            .find(|map| map.path == map_path)
            .unwrap();
        assert!(map.url.is_none());
        let decoded = sourcemap::SourceMap::from_slice(&output.files[&map.path]).unwrap();
        assert!(decoded.get_token_count() > 0);
        has_svelte_source |= (0..decoded.get_source_count()).any(|index| {
            decoded
                .get_source(index)
                .is_some_and(|source| source.ends_with("SvelteApp.svelte"))
        });
    }
    assert!(has_svelte_source);
    assert!(output.files.keys().all(|path| !path.ends_with(".css.map")));

    let forbidden = generated.join("forbidden.js");
    let mut bad_config = config.clone();
    bad_config.server_entry = forbidden;
    for module in ["node:async_hooks", "node:fs"] {
        fs::write(
            &bad_config.server_entry,
            format!(
                "import '{module}'; export function render() {{ return {{html:'',head:'',state:null}}; }}"
            ),
        )
        .unwrap();
        assert!(build(&bad_config).await.is_err(), "{module} must fail");
    }
}
