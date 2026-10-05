mod browser;
#[path = "../../ssr-build/tests/fixture/mod.rs"]
mod fixture;
mod support;
use ssr_adapter_vanilla::{VanillaAdapter, client_entry, server_entry};
use ssr_build::{Build, BuildConfig, build};
use ssr_core::Page;
use ssr_runtime::{Pool, ServerBundle};
use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

fn page(render: &str) -> Page {
    Page::from_json(format!(r#"{{"render":"{render}","title":"Example","language":"en","props":{{"name":"Ada"}},"state":{{"count":4}}}}"#).as_bytes()).unwrap()
}

fn shell_page() -> Page {
    Page::from_json(
        br#"{"render":"csr","title":"Example","language":"en","props":{},"state":null}"#,
    )
    .unwrap()
}

fn fixture(entry_bytes: &[u8]) -> ServerBundle {
    ServerBundle {
        entry_path: "server/fixture.js".into(),
        entry_bytes: entry_bytes.to_vec(),
        chunks: Vec::new(),
    }
}

fn built_bundle(build: &Build) -> ServerBundle {
    ServerBundle {
        entry_path: build.manifest.server.path.clone(),
        entry_bytes: build.files[&build.manifest.server.path].clone(),
        chunks: build
            .manifest
            .server_chunks
            .iter()
            .map(|file| (file.path.clone(), build.files[&file.path].clone()))
            .collect(),
    }
}

#[test]
fn entries_render_both_modes_and_reject_invalid_input() {
    let server = server_entry("/application/server.js").unwrap();
    assert!(server.contains("export function render(props, state)"));
    assert!(!server.contains("globalThis.render"));
    assert!(
        client_entry("/application/client.js")
            .unwrap()
            .contains("hydrate")
    );
    let client = client_entry("/application/client.js").unwrap();
    assert!(client.contains("root.dataset.render === 'ssr'"));
    assert!(!client.contains("hasChildNodes"));
    assert!(server_entry("relative/server.js").is_err());
    assert!(client_entry("relative/client.js").is_err());

    let pool = Pool::new(fixture(b"export function render(props, state) { return {head:'', html: '<main>' + props.name + '</main>', state: {count: state.count + 1}}; }"), support::options(1, 0, Duration::from_secs(2))).unwrap();
    let adapter = VanillaAdapter::new("/assets/client.js", &["/assets/style.css"]).unwrap();
    let ssr = adapter.render(&page("ssr"), &pool).unwrap();
    assert_eq!(ssr.state.compact(), r#"{"count":5}"#);
    assert!(
        String::from_utf8(ssr.html)
            .unwrap()
            .contains("<main>Ada</main>")
    );
    let csr = adapter.render(&page("csr"), &pool).unwrap();
    assert_eq!(csr.state.compact(), r#"{"count":4}"#);
    assert!(
        String::from_utf8(csr.html)
            .unwrap()
            .contains("<div id=\"root\" data-render=\"csr\"></div>")
    );
    assert!(adapter.static_shell(&page("csr")).is_err());
    assert_eq!(
        adapter.static_shell(&shell_page()).unwrap(),
        adapter.static_shell(&shell_page()).unwrap()
    );
    assert!(VanillaAdapter::new("//other/client.js", &[]).is_err());
    assert!(VanillaAdapter::new("/assets/client.js", &["/assets/../style.css"]).is_err());
}

#[test]
fn empty_ssr_document_preserves_render_mode() {
    let pool = Pool::new(
        fixture(b"export function render(props, state) { return {head:'', html:'', state}; }"),
        support::options(1, 0, Duration::from_secs(2)),
    )
    .unwrap();
    let adapter = VanillaAdapter::new("/assets/client.js", &["/assets/style.css"]).unwrap();
    let empty = adapter.render(&page("ssr"), &pool).unwrap();
    assert!(
        String::from_utf8(empty.html)
            .unwrap()
            .contains("<div id=\"root\" data-render=\"ssr\"></div>")
    );
}

#[test]
fn server_head_is_rejected() {
    let pool = Pool::new(
        fixture(b"export function render(props, state) { return {html:'ok', head:'<script>unsafe()</script>', state}; }"),
        support::options(1, 0, Duration::from_secs(2)),
    ).unwrap();
    let adapter = VanillaAdapter::new("/assets/client.js", &["/assets/style.css"]).unwrap();
    assert!(adapter.render(&page("ssr"), &pool).is_err());
}

#[tokio::test]
async fn browser_preserves_server_dom_and_renders_csr_and_shells() {
    assert!(
        Path::new(browser::BROWSER).is_file(),
        "browser test environment missing: {}",
        browser::BROWSER
    );
    let fixture = fixture::Fixture::new();
    let root = fixture.root.clone();
    let generated = root.join("generated-ssr-adapter-vanilla");
    fs::create_dir_all(&generated).unwrap();
    let application = root.join("VanillaApp.js");
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
    let pool = Pool::new(
        built_bundle(&output),
        support::options(1, 0, Duration::from_secs(10)),
    )
    .unwrap();
    let adapter = VanillaAdapter::new(
        output.manifest.client.url.as_ref().unwrap(),
        &output
            .manifest
            .styles
            .iter()
            .map(|style| style.url.as_deref().unwrap())
            .collect::<Vec<_>>(),
    )
    .unwrap();
    let mut responses = BTreeMap::<String, (String, Vec<u8>)>::new();
    for file in std::iter::once(&output.manifest.client)
        .chain(output.manifest.styles.iter())
        .chain(output.manifest.assets.iter())
    {
        responses.insert(
            file.url.as_ref().unwrap().clone(),
            (file.content_type.clone(), output.files[&file.path].clone()),
        );
    }
    let ssr = adapter.render(&page("ssr"), &pool).unwrap();
    assert_eq!(ssr.state.compact(), r#"{"count":5}"#);
    let ssr_html = String::from_utf8(ssr.html).unwrap().replace(
        "</div><script id=\"__SSR_PROPS__\"",
        "</div><script>window.__before = document.querySelector('#root main');</script><script id=\"__SSR_PROPS__\"",
    );
    responses.insert(
        "/ssr".into(),
        ("text/html; charset=utf-8".into(), ssr_html.into_bytes()),
    );
    let empty_page = Page::from_json(br#"{"render":"ssr","title":"Example","language":"en","props":{"name":"Ada","empty":true},"state":{"count":4}}"#).unwrap();
    let empty = adapter.render(&empty_page, &pool).unwrap();
    let empty_html = String::from_utf8(empty.html).unwrap();
    assert!(empty_html.contains("<div id=\"root\" data-render=\"ssr\"></div>"));
    responses.insert(
        "/empty-ssr".into(),
        ("text/html; charset=utf-8".into(), empty_html.into_bytes()),
    );
    let csr = adapter.render(&page("csr"), &pool).unwrap();
    assert_eq!(csr.state.compact(), r#"{"count":4}"#);
    let csr_html = String::from_utf8(csr.html.clone()).unwrap();
    for (path, marker) in [
        ("/missing-mode", ""),
        ("/invalid-mode", " data-render=\"other\""),
    ] {
        let invalid = csr_html.replace(" data-render=\"csr\"", marker);
        assert_ne!(invalid, csr_html);
        responses.insert(
            path.into(),
            ("text/html; charset=utf-8".into(), invalid.into_bytes()),
        );
    }
    responses.insert("/csr".into(), ("text/html; charset=utf-8".into(), csr.html));
    let shell = adapter.static_shell(&shell_page()).unwrap();
    for path in ["/shell/first", "/shell/second"] {
        responses.insert(
            path.into(),
            ("text/html; charset=utf-8".into(), shell.clone()),
        );
    }

    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let address = listener.local_addr().unwrap();
    let responses = Arc::new(responses);
    let stopping = Arc::new(AtomicBool::new(false));
    let server_stopping = Arc::clone(&stopping);
    let server = thread::spawn(move || {
        let mut handlers = Vec::new();
        for incoming in listener.incoming() {
            if server_stopping.load(Ordering::Acquire) {
                break;
            }
            let responses = Arc::clone(&responses);
            let stopping = Arc::clone(&server_stopping);
            handlers.push(thread::spawn(move || {
                let mut stream = incoming.unwrap();
                let mut line = String::new();
                match BufReader::new(&stream).read_line(&mut line) {
                    Ok(0) => return,
                    Ok(_) => {},
                    Err(error) => panic!("browser request read failed: {error}"),
                }
                let path = line.split_whitespace().nth(1).expect("browser request path");
                if path == "/__stop" {
                    stopping.store(true, Ordering::Release);
                    stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
                    return;
                }
                let (status, content_type, bytes) = match responses.get(path) {
                    Some((content_type, bytes)) => ("200 OK", content_type.as_str(), bytes.as_slice()),
                    None => ("404 Not Found", "text/plain; charset=utf-8", b"Not found".as_slice()),
                };
                let header = format!("HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", bytes.len());
                stream.write_all(header.as_bytes()).unwrap();
                stream.write_all(bytes).unwrap();
            }));
        }
        for handler in handlers {
            handler.join().unwrap();
        }
    });
    let output = browser::run(
        &root.join("browser-vanilla.mjs"),
        &format!("http://{address}"),
        &[],
    );
    assert_eq!(output.matches("PASS ").count(), 7);
    let mut stop = std::net::TcpStream::connect(address).unwrap();
    stop.write_all(b"GET /__stop HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
        .unwrap();
    stop.read_exact(&mut [0_u8; 1]).unwrap();
    std::net::TcpStream::connect(address).unwrap();
    server.join().unwrap();
}
