mod fixture;
use ssr_build::{BuildConfig, Error, build};

#[tokio::test]
async fn compiler_stops_before_v8_after_renderer_selection() {
    let fixture = fixture::Fixture::new();
    let root = fixture.root.clone();
    let generated = root.join("generated-ssr-build-process");
    std::fs::create_dir_all(&generated).unwrap();
    let server = generated.join("server.js");
    let client = generated.join("client.js");
    let source = "import App from '../SvelteApp.svelte'; export const render = () => App;";
    std::fs::write(&server, source).unwrap();
    std::fs::write(&client, source).unwrap();
    ssr_core::process::render(|| Ok::<_, ()>(()))
        .unwrap()
        .unwrap();
    let config = BuildConfig {
        root: root.clone(),
        server_entry: server.clone(),
        client_entry: client.clone(),
        react_framework_entry: None,
        css_entry: root.join("app.css"),
        asset_route: "/assets".into(),
        dependencies: Some(fixture.packages.clone()),
    };
    let result = build(&config).await;
    assert!(
        matches!(result, Err(Error::JavaScript(message)) if message.contains("Svelte compilation cannot run after renderer initialization"))
    );
    std::fs::write(server, "export function render() { return 'server'; }").unwrap();
    std::fs::write(client, "globalThis.client = true;").unwrap();
    let ordinary = build(&config).await.unwrap();
    assert!(!ordinary.files.is_empty());
}
