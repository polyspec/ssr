mod support;
use ssr_adapter_react::{client_entry, framework_entry, server_entry};
use ssr_build::{BuildConfig, build};
use ssr_core::Page;
use ssr_runtime::{Pool, ServerBundle};
use std::fs;
use std::path::Path;
use std::time::Duration;

const GENERATED_ENTRY_DIR: &str = "node_modules/.ssr-adapter-react-heap";

// Each stream render runs in its own context of one isolate; a context that
// stays reachable after its render keeps its heap, so repeated renders grow
// the heap without bound.
#[tokio::test]
async fn repeated_stream_renders_keep_the_heap_bounded() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/build-probe/tests/fixtures")
        .canonicalize()
        .unwrap();
    assert!(
        root.join("node_modules/react").is_dir(),
        "install sample packages before tests"
    );
    let generated = root.join(GENERATED_ENTRY_DIR);
    fs::create_dir_all(&generated).unwrap();
    let application = root.join("ReactApp.tsx");
    let application = application.to_str().unwrap();
    fs::write(
        generated.join("server.tsx"),
        server_entry(application).unwrap(),
    )
    .unwrap();
    fs::write(generated.join("framework.tsx"), framework_entry()).unwrap();
    fs::write(
        generated.join("client.tsx"),
        client_entry(application).unwrap(),
    )
    .unwrap();
    let output = build(&BuildConfig {
        root: root.clone(),
        server_entry: generated.join("server.tsx"),
        react_framework_entry: Some(generated.join("framework.tsx")),
        client_entry: generated.join("client.tsx"),
        css_entry: root.join("app.css"),
        asset_route: "/assets".into(),
        dependencies: None,
    })
    .await
    .unwrap();
    let framework = output.manifest.react_framework.as_ref().unwrap();
    let pool = Pool::new_react(
        (
            framework.path.clone(),
            output.files[&framework.path].clone(),
        ),
        ServerBundle {
            entry_path: output.manifest.server.path.clone(),
            entry_bytes: output.files[&output.manifest.server.path].clone(),
            chunks: Vec::new(),
        },
        support::options(1, 0, Duration::from_secs(10)),
    )
    .unwrap();
    let page = Page::from_json(
        br#"{"render":"ssr","title":"Example","language":"en","props":{"name":"Ada"},"state":{"count":4}}"#,
    )
    .unwrap();
    let mut heap = Vec::new();
    for _ in 0..600 {
        let (_, stream) = pool
            .render_stream(&page, "sample_nonce", ssr_runtime::Cancellation::new())
            .unwrap();
        heap.push(stream.metrics().heap_used_bytes);
        stream.collect::<Result<Vec<_>, _>>().unwrap();
    }
    let early = *heap[..50].iter().max().unwrap();
    let late = *heap[550..].iter().min().unwrap();
    assert!(
        late < early + 16 * 1024 * 1024,
        "heap grew from {early} to {late} bytes over 600 renders"
    );
}
