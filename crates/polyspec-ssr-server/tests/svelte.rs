#[path = "../../polyspec-ssr-build/tests/fixture/mod.rs"]
mod fixture;
mod support;
use http::{Method, Request, StatusCode};
use polyspec_ssr_adapter_svelte::{client_entry, server_entry};
use polyspec_ssr_build::{BuildConfig, build};
use polyspec_ssr_server::{Adapter, Server};
use std::fs;
use std::time::Duration;

const SSR: &[u8] = br#"{"render":"ssr","title":"Svelte","language":"en","props":{"name":"Ada"},"state":{"count":4}}"#;

#[tokio::test]
async fn svelte_http_preserves_head_css_and_request_nonce() {
    let fixture = fixture::Fixture::new();
    let root = fixture.root.clone();
    let generated = root.join("generated-ssr-server-svelte");
    fs::create_dir_all(&generated).unwrap();
    let application = root.join("SvelteApp.svelte");
    let server_path = generated.join("server.js");
    let client_path = generated.join("client.js");
    fs::write(
        &server_path,
        server_entry(application.to_str().unwrap()).unwrap(),
    )
    .unwrap();
    fs::write(
        &client_path,
        client_entry(application.to_str().unwrap()).unwrap(),
    )
    .unwrap();
    let output = build(&BuildConfig {
        root: root.clone(),
        server_entry: server_path,
        react_framework_entry: None,
        client_entry: client_path,
        css_entry: root.join("app.css"),
        asset_route: "/assets".into(),
        dependencies: Some(fixture.packages.clone()),
    })
    .await
    .unwrap();
    let server = Server::new(
        &output,
        Adapter::Svelte,
        support::options(1, 0, Duration::from_secs(10)),
    )
    .unwrap();
    let request = Request::builder()
        .method(Method::POST)
        .uri("/_render")
        .header("content-type", "application/json")
        .body(SSR.to_vec())
        .unwrap();
    let response = server.handle(request).unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let html = String::from_utf8(response.into_body().collect_bytes().unwrap()).unwrap();
    assert!(
        html.contains("<meta name=\"svelte-head\" content=\"Ada\""),
        "{html}"
    );
    assert!(html.contains("<h1>Ada</h1>"), "{html}");
    assert!(html.contains("window.__svelteHead = true;"), "{html}");
    assert!(
        html.contains(".svelte-head-style { color: #abcdef; }"),
        "{html}"
    );
    let nonce = html
        .split(" nonce=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    assert_eq!(nonce.len(), 32);
    assert!(nonce.bytes().all(|byte| byte.is_ascii_hexdigit()));
    assert_eq!(
        html.matches("<script").count() + html.matches("<style").count(),
        html.matches(&format!("nonce=\"{nonce}\"")).count()
    );
    let component_css = output
        .manifest
        .styles
        .iter()
        .find(|style| {
            output.files[&style.path]
                .windows(7)
                .any(|part| part == b"#123456")
        })
        .unwrap();
    let css_url = component_css.url.as_ref().unwrap();
    assert!(html.contains(&format!("href=\"{css_url}\"")));
    let css_request = Request::builder()
        .method(Method::GET)
        .uri(css_url)
        .body(Vec::new())
        .unwrap();
    let css_response = server.handle(css_request).unwrap();
    assert_eq!(css_response.status(), StatusCode::OK);
    assert_eq!(
        css_response.into_body().collect_bytes().unwrap(),
        output.files[&component_css.path]
    );
}
