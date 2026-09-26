use http::header::{ALLOW, CACHE_CONTROL, CONTENT_TYPE};
use http::{Method, Request, StatusCode};
use sha2::{Digest, Sha256};
use sourcemap::SourceMapBuilder;
use ssr_build::{Build, BuildConfig, BuildFile, Manifest, build as build_application};
use ssr_server::{Adapter, Server};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn file(path: &str, url: Option<&str>, bytes: &[u8], content_type: &str) -> BuildFile {
    BuildFile {
        path: path.into(),
        url: url.map(str::to_owned),
        sha256: Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
        content_type: content_type.into(),
    }
}

fn build(script: &str) -> Build {
    let mut builder = SourceMapBuilder::new(Some("server/server.js"));
    builder.add(0, 0, 6, 2, Some("src/page.tsx"), None, false);
    let mut map = Vec::new();
    builder.into_sourcemap().to_writer(&mut map).unwrap();
    let script = script.as_bytes().to_vec();
    let client = b"window.app = true;".to_vec();
    Build {
        manifest: Manifest {
            react_framework: None,
            server: file(
                "server/server.js",
                None,
                &script,
                "text/javascript; charset=utf-8",
            ),
            source_maps: vec![file(
                "server/server.js.map",
                None,
                &map,
                "application/json; charset=utf-8",
            )],
            client: file(
                "client/app.js",
                Some("/assets/app.js"),
                &client,
                "text/javascript; charset=utf-8",
            ),
            styles: Vec::new(),
            server_chunks: Vec::new(),
            assets: Vec::new(),
        },
        files: BTreeMap::from([
            ("server/server.js".into(), script),
            ("server/server.js.map".into(), map),
            ("client/app.js".into(), client),
        ]),
    }
}

fn with_chunk(script: &str, chunk: &str) -> Build {
    let mut output = build(script);
    let path = "server/chunk.js";
    let bytes = chunk.as_bytes().to_vec();
    output
        .manifest
        .server_chunks
        .push(file(path, None, &bytes, "text/javascript; charset=utf-8"));
    output.files.insert(path.into(), bytes);
    let map_path = "server/chunk.js.map";
    let mut builder = SourceMapBuilder::new(Some(path));
    builder.add(0, 0, 3, 2, Some("src/chunk.ts"), None, false);
    let mut map = Vec::new();
    builder.into_sourcemap().to_writer(&mut map).unwrap();
    output.manifest.source_maps.push(file(
        map_path,
        None,
        &map,
        "application/json; charset=utf-8",
    ));
    output.files.insert(map_path.into(), map);
    output
}

fn request(method: Method, path: &str, body: &[u8], json: bool) -> Request<Vec<u8>> {
    let mut builder = Request::builder().method(method).uri(path);
    if json {
        builder = builder.header(CONTENT_TYPE, "application/json");
    }
    builder.body(body.to_vec()).unwrap()
}

const SSR: &[u8] = br#"{"render":"ssr","title":"Page","language":"en","props":{"name":"Ada"},"state":{"count":3}}"#;
const CSR: &[u8] = br#"{"render":"csr","title":"Page","language":"en","props":{"name":"Ada"},"state":{"count":3}}"#;

#[derive(Clone)]
struct TraceOutput(Arc<Mutex<Vec<u8>>>);

impl Write for TraceOutput {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn render_http_maps_status_bodies_and_metrics() {
    let build = build(
        "export function render(props, state) { return {head:'', html:'<h1>'+props.name+'</h1>', state}; }",
    );
    let server = Server::new(&build, Adapter::React, 1, 1, Duration::from_secs(2)).unwrap();
    let trace_bytes = Arc::new(Mutex::new(Vec::new()));
    let writer = TraceOutput(trace_bytes.clone());
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_writer(move || writer.clone())
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);

    let ssr = server
        .handle(request(Method::POST, "/_render", SSR, true))
        .unwrap();
    assert_eq!(ssr.status(), StatusCode::OK);
    assert_eq!(ssr.headers()[CONTENT_TYPE], "text/html; charset=utf-8");
    assert_eq!(ssr.headers()[CACHE_CONTROL], "no-store");
    assert!(
        std::str::from_utf8(ssr.body())
            .unwrap()
            .contains("<h1>Ada</h1>")
    );
    let trace = String::from_utf8(trace_bytes.lock().unwrap().clone()).unwrap();
    assert!(trace.contains("render_ms="), "{trace}");
    assert!(trace.contains("pool_wait_ms="), "{trace}");
    assert!(trace.contains("heap_used_bytes="), "{trace}");

    let csr = server
        .handle(request(Method::POST, "/_render", CSR, true))
        .unwrap();
    assert_eq!(csr.status(), StatusCode::OK);
    assert!(
        !std::str::from_utf8(csr.body())
            .unwrap()
            .contains("<h1>Ada</h1>")
    );
    let asset = server
        .handle(request(Method::GET, "/assets/app.js", &[], false))
        .unwrap();
    assert_eq!(asset.status(), StatusCode::OK);
    assert_eq!(asset.body(), b"window.app = true;");
    let bad = server
        .handle(request(Method::POST, "/_render", b"{", true))
        .unwrap();
    assert_eq!(bad.status(), StatusCode::BAD_REQUEST);
    assert!(!bad.body().is_empty());
    assert_eq!(
        server
            .handle(request(Method::POST, "/_render", SSR, false))
            .unwrap()
            .status(),
        StatusCode::UNSUPPORTED_MEDIA_TYPE
    );
    let wrong_method = server
        .handle(request(Method::GET, "/_render", &[], false))
        .unwrap();
    assert_eq!(wrong_method.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(wrong_method.headers()[ALLOW], "POST");
}

#[test]
fn javascript_stack_uses_source_map_and_invalid_map_fails_startup() {
    let output = build("export function render() { throw new Error('boom'); }");
    let server = Server::new(&output, Adapter::React, 1, 0, Duration::from_secs(2)).unwrap();
    let trace_bytes = Arc::new(Mutex::new(Vec::new()));
    let writer = TraceOutput(trace_bytes.clone());
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_writer(move || writer.clone())
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);
    let response = server
        .handle(request(Method::POST, "/_render", SSR, true))
        .unwrap();
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let text = std::str::from_utf8(response.body()).unwrap();
    assert!(text.contains("boom"), "{text}");
    assert!(text.contains("src/page.tsx:7:3"), "{text}");
    let trace = String::from_utf8(trace_bytes.lock().unwrap().clone()).unwrap();
    assert!(trace.contains("src/page.tsx:7:3"), "{trace}");
    let mut invalid = output;
    invalid.manifest.source_maps.clear();
    assert!(Server::new(&invalid, Adapter::React, 1, 0, Duration::from_secs(2)).is_err());

    let mut invalid = build("export function render() { throw new Error('boom'); }");
    invalid.manifest.source_maps[0].sha256 = "invalid".into();
    assert!(Server::new(&invalid, Adapter::React, 1, 0, Duration::from_secs(2)).is_err());
}

#[test]
fn static_and_dynamic_chunk_failures_use_their_own_source_map() {
    let chunk = "export function fail() { throw new Error('chunk boom'); }";
    for entry in [
        "import { fail } from './chunk.js'; export function render() { fail(); }",
        "export async function render() { const chunk = await import('./chunk.js'); chunk.fail(); }",
    ] {
        let output = with_chunk(entry, chunk);
        let server = Server::new(&output, Adapter::React, 1, 0, Duration::from_secs(2)).unwrap();
        let response = server
            .handle(request(Method::POST, "/_render", SSR, true))
            .unwrap();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let text = std::str::from_utf8(response.body()).unwrap();
        assert!(text.contains("chunk boom"), "{text}");
        assert!(text.contains("src/chunk.ts:4:3"), "{text}");
        assert!(!text.contains("stack mapping failed"), "{text}");
    }
}

#[test]
fn missing_or_changed_private_chunk_fails_construction() {
    let mut output = with_chunk(
        "export async function render() { await import('./chunk.js'); }",
        "export const value = 1;",
    );
    output.files.remove("server/chunk.js");
    assert!(Server::new(&output, Adapter::React, 1, 0, Duration::from_secs(2)).is_err());

    let mut output = with_chunk(
        "export async function render() { await import('./chunk.js'); }",
        "export const value = 1;",
    );
    output.manifest.server_chunks[0].sha256 = "invalid".into();
    assert!(Server::new(&output, Adapter::React, 1, 0, Duration::from_secs(2)).is_err());

    let mut output = with_chunk(
        "export async function render() { await import('./chunk.js'); }",
        "export const value = 1;",
    );
    output.manifest.source_maps.pop();
    assert!(Server::new(&output, Adapter::React, 1, 0, Duration::from_secs(2)).is_err());
}

#[test]
fn standard_utf8_json_media_type_and_head_semantics() {
    let build =
        build("export function render(props, state) { return {head:'', html:'ok', state}; }");
    let server = Server::new(&build, Adapter::React, 1, 0, Duration::from_secs(2)).unwrap();
    let json_request = Request::builder()
        .method(Method::POST)
        .uri("/_render")
        .header(CONTENT_TYPE, "application/json; charset=utf-8")
        .body(SSR.to_vec())
        .unwrap();
    assert_eq!(
        server.handle(json_request).unwrap().status(),
        StatusCode::OK
    );
    let head = server
        .handle(request(Method::HEAD, "/_render", &[], false))
        .unwrap();
    assert_eq!(head.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert!(head.body().is_empty());
    let query = server
        .handle(request(Method::POST, "/_render?unused=1", SSR, true))
        .unwrap();
    assert_eq!(query.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn generated_source_map_maps_a_live_render_failure() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/build-probe/tests/fixtures")
        .canonicalize()
        .unwrap();
    assert!(
        root.join("node_modules/react").is_dir(),
        "sample packages are required"
    );
    let output = build_application(&BuildConfig {
        server_entry: root.join("server_error.tsx"),
        react_framework_entry: None,
        client_entry: root.join("client.tsx"),
        css_entry: root.join("app.css"),
        asset_route: "/assets".into(),
        root,
    })
    .await
    .unwrap();
    let server = Server::new(&output, Adapter::React, 1, 0, Duration::from_secs(10)).unwrap();
    let response = server
        .handle(request(Method::POST, "/_render", SSR, true))
        .unwrap();
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let text = std::str::from_utf8(response.body()).unwrap();
    assert!(text.contains("server_error.tsx"), "{text}");
    assert!(text.contains("mapped render failure"), "{text}");
}
