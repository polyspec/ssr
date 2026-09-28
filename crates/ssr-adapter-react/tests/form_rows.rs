mod support;
use ssr_adapter_react::{ReactAdapter, client_entry, framework_entry, server_entry};
use ssr_build::{BuildConfig, build};
use ssr_core::Page;
use ssr_runtime::{Pool, ServerBundle};
use std::fs;
use std::path::Path;
use std::time::Duration;

fn row_key(document: &str, prefix: &str, suffix: &str) -> String {
    for (offset, _) in document.match_indices(prefix) {
        let remainder = &document[offset + prefix.len()..];
        if let Some(end) = remainder.find('.')
            && remainder[end..].starts_with(suffix)
        {
            let key = &remainder[..end];
            assert_eq!(key.len(), 12, "row key length: {key}");
            assert!(key.starts_with("row-"));
            assert!(key[4..].bytes().all(|byte| byte.is_ascii_hexdigit()));
            return key.to_owned();
        }
    }
    panic!("row control missing: {prefix}...{suffix}");
}

#[tokio::test]
async fn repeated_form_rows_have_distinct_keys_across_two_server_renders() {
    // FormRowsApp.tsx renders a repeated row group with a repeated child group; each row key comes
    // from crypto.getRandomValues during the render.
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/build-probe/tests/fixtures")
        .canonicalize()
        .unwrap();
    assert!(
        root.join("node_modules/react").is_dir(),
        "install sample packages before tests"
    );
    let generated = root.join("node_modules/.ssr-adapter-react-form-rows");
    fs::create_dir_all(&generated).unwrap();
    let application = root.join("FormRowsApp.tsx");
    let server_path = generated.join("server.tsx");
    let framework_path = generated.join("framework.tsx");
    let client_path = generated.join("client.tsx");
    fs::write(
        &server_path,
        server_entry(application.to_str().unwrap()).unwrap(),
    )
    .unwrap();
    fs::write(&framework_path, framework_entry()).unwrap();
    fs::write(
        &client_path,
        client_entry(application.to_str().unwrap()).unwrap(),
    )
    .unwrap();
    let output = build(&BuildConfig {
        root: root.clone(),
        server_entry: server_path,
        react_framework_entry: Some(framework_path),
        client_entry: client_path,
        css_entry: root.join("app.css"),
        asset_route: "/assets".into(),
        dependencies: None,
    })
    .await
    .unwrap();
    let framework = output.manifest.react_framework.as_ref().unwrap();
    assert!(output.manifest.server_chunks.is_empty());
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
    let adapter = ReactAdapter::new(
        output.manifest.client.url.as_ref().unwrap(),
        &output
            .manifest
            .styles
            .iter()
            .map(|style| style.url.as_deref().unwrap())
            .collect::<Vec<_>>(),
    )
    .unwrap();
    let page = Page::from_json(
        br#"{"render":"ssr","title":"Form","language":"en","props":{},"state":null}"#,
    )
    .unwrap();
    let mut previous = None;
    for _ in 0..2 {
        let (state, stream) = pool
            .render_stream(&page, "form_rows_nonce", ssr_runtime::Cancellation::new())
            .unwrap();
        let (mut prefix, suffix) = adapter.stream_parts(&page, &state).unwrap();
        prefix.extend_from_slice(&stream.collect::<Result<Vec<_>, _>>().unwrap().concat());
        prefix.extend_from_slice(&suffix);
        let document = String::from_utf8(prefix).unwrap();
        let parent = row_key(&document, "name=\"rows.", ".title\"");
        let child = row_key(
            &document,
            &format!("name=\"rows.{parent}.children."),
            ".title\"",
        );
        assert_ne!(parent, child);
        if let Some(previous) = &previous {
            assert_ne!(previous, &parent);
        }
        previous = Some(parent);
    }
}
