mod support;
use ssr_adapter_react::{ReactAdapter, client_entry, framework_entry, server_entry};
use ssr_build::{Build, BuildConfig, build};
use ssr_core::{Page, RenderResult};
use ssr_runtime::{Pool, ServerBundle};
use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::Barrier;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;
use wait_timeout::ChildExt;

const GENERATED_ENTRY_DIR: &str = "node_modules/.ssr-adapter-react-entry";
const GENERATED_BROWSER_DIR: &str = "node_modules/.ssr-adapter-react-browser";

#[test]
fn concurrent_react_tests_keep_their_generated_sources() {
    fn prepare(path: &Path, marker: &str, start: &Barrier, written: &Barrier) -> [String; 3] {
        fs::create_dir_all(path).unwrap();
        start.wait();
        for name in ["server.tsx", "framework.tsx", "client.tsx"] {
            fs::write(path.join(name), format!("{marker}:{name}")).unwrap();
        }
        written.wait();
        ["server.tsx", "framework.tsx", "client.tsx"]
            .map(|name| fs::read_to_string(path.join(name)).unwrap())
    }

    let root = std::env::temp_dir().join(format!("ssr-react-test-{}", std::process::id()));
    let entry = root.join(GENERATED_ENTRY_DIR);
    let browser = root.join(GENERATED_BROWSER_DIR);
    let start = Barrier::new(2);
    let written = Barrier::new(2);
    let (entry_sources, browser_sources) = thread::scope(|scope| {
        let entry_worker = scope.spawn(|| prepare(&entry, "entry", &start, &written));
        let browser_worker = scope.spawn(|| prepare(&browser, "browser", &start, &written));
        (entry_worker.join().unwrap(), browser_worker.join().unwrap())
    });
    fs::remove_dir_all(root).unwrap();
    assert_eq!(
        entry_sources,
        [
            "entry:server.tsx",
            "entry:framework.tsx",
            "entry:client.tsx"
        ]
    );
    assert_eq!(
        browser_sources,
        [
            "browser:server.tsx",
            "browser:framework.tsx",
            "browser:client.tsx"
        ]
    );
}

fn page(render: &str) -> Page {
    Page::from_json(format!(r#"{{"render":"{render}","title":"Example","language":"en","props":{{"name":"Ada"}},"state":{{"count":4}}}}"#).as_bytes()).unwrap()
}

fn shell_page() -> Page {
    Page::from_json(
        br#"{"render":"csr","title":"Example","language":"en","props":{},"state":null}"#,
    )
    .unwrap()
}

fn stream_document(adapter: &ReactAdapter, page: &Page, pool: &Pool) -> RenderResult {
    let (state, stream) = pool
        .render_stream(page, "sample_nonce", ssr_runtime::Cancellation::new())
        .unwrap();
    let (mut prefix, suffix) = adapter.stream_parts(page, &state).unwrap();
    let body = stream.collect::<Result<Vec<_>, _>>().unwrap().concat();
    prefix.extend_from_slice(&body);
    prefix.extend_from_slice(&suffix);
    RenderResult {
        html: prefix,
        head: Vec::new(),
        state,
    }
}

fn react_pool(build: &Build) -> Pool {
    assert!(build.manifest.server_chunks.is_empty());
    let framework = build.manifest.react_framework.as_ref().unwrap();
    let app_source = std::str::from_utf8(&build.files[&build.manifest.server.path]).unwrap();
    assert!(!app_source.contains("require(\"react\")"));
    Pool::new_react(
        (framework.path.clone(), build.files[&framework.path].clone()),
        ServerBundle {
            entry_path: build.manifest.server.path.clone(),
            entry_bytes: build.files[&build.manifest.server.path].clone(),
            chunks: Vec::new(),
        },
        support::options(1, 0, Duration::from_secs(10)),
    )
    .unwrap()
}

#[test]
fn server_result_and_csr_state_follow_the_same_client_build() {
    let server = server_entry("/application/App.tsx").unwrap();
    let client = client_entry("/application/App.tsx").unwrap();
    assert!(server.contains("__ssrApp"));
    assert!(framework_entry().contains("renderToReadableStream"));
    assert!(client.contains("hydrateRoot"));
    assert!(client.contains("createRoot"));
    assert!(client.contains("root.dataset.render === 'ssr'"));
    assert!(!client.contains("hasChildNodes"));

    let adapter = ReactAdapter::new("/assets/client.js", &["/assets/style.css"]).unwrap();
    assert!(adapter.render(&page("ssr")).is_err());
    let csr = adapter.render(&page("csr")).unwrap();
    assert_eq!(csr.state.compact(), r#"{"count":4}"#);
    let csr_html = String::from_utf8(csr.html).unwrap();
    assert!(csr_html.contains("<div id=\"root\" data-render=\"csr\"></div>"));
    assert!(csr_html.contains("/assets/client.js"));
    assert!(!csr_html.contains("<main>Ada</main>"));
    assert_eq!(
        adapter.static_shell(&shell_page()).unwrap(),
        adapter.static_shell(&shell_page()).unwrap()
    );
}

#[test]
fn invalid_entry_urls_and_shell_values_fail() {
    assert!(server_entry("relative/App.tsx").is_err());
    assert!(client_entry("relative/App.tsx").is_err());
    for client in [
        "",
        "//other/client.js",
        "/assets/../client.js",
        "/assets/client.js?x=1",
        "/assets/client.css",
    ] {
        assert!(ReactAdapter::new(client, &[]).is_err(), "{client}");
    }
    for style in [
        "//other/style.css",
        "/assets/../style.css",
        "/assets/style.css?x=1",
        "/assets/style.js",
    ] {
        assert!(
            ReactAdapter::new("/assets/client.js", &[style]).is_err(),
            "{style}"
        );
    }
    let adapter = ReactAdapter::new("/assets/client.js", &[]).unwrap();
    assert!(adapter.static_shell(&page("csr")).is_err());
    assert!(adapter.static_shell(&page("ssr")).is_err());
    assert!(adapter.render(&page("ssr")).is_err());
}

#[tokio::test]
async fn generated_react_entries_build_and_execute() {
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
    let pool = react_pool(&output);
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
    let ssr = stream_document(&adapter, &page("ssr"), &pool);
    assert_eq!(ssr.state.compact(), r#"{"count":5}"#);
    let html = String::from_utf8(ssr.html).unwrap();
    assert!(html.contains("<h1>Ada</h1>"), "{html}");
    let csr = adapter.render(&page("csr")).unwrap();
    assert_eq!(csr.state.compact(), r#"{"count":4}"#);
}

#[tokio::test]
async fn suspense_failure_keeps_fallback_and_client_recovery_in_stream() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/build-probe/tests/fixtures")
        .canonicalize()
        .unwrap();
    assert!(
        root.join("node_modules/react").is_dir(),
        "sample packages are required"
    );
    let generated = root.join("node_modules/.ssr-adapter-react-stream");
    fs::create_dir_all(&generated).unwrap();
    let application = root.join("ReactStreamApp.tsx");
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
    let pool = react_pool(&output);
    let page = Page::from_json(br#"{"render":"ssr","title":"Stream","language":"en","props":{"failClient":false},"state":{"count":4}}"#).unwrap();
    let (state, mut stream) = pool
        .render_stream(&page, "stream_nonce", ssr_runtime::Cancellation::new())
        .unwrap();
    assert_eq!(state.compact(), r#"{"count":4}"#);
    let shell = String::from_utf8(stream.next().unwrap().unwrap()).unwrap();
    assert!(shell.contains("<h1>Ready</h1>"), "{shell}");
    assert!(shell.contains("id=\"fallback\""), "{shell}");
    let remaining =
        String::from_utf8(stream.collect::<Result<Vec<_>, _>>().unwrap().concat()).unwrap();
    let document = format!("{shell}{remaining}");
    assert!(
        document.contains("<!--$!--><template data-msg="),
        "{document}"
    );
    assert!(document.contains("late server failure"), "{document}");
}

#[cfg(target_os = "macos")]
const BROWSER: &str = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
#[cfg(target_os = "linux")]
const BROWSER: &str = "/usr/bin/google-chrome";

fn run_browser(base: &str, script: &Path) {
    assert!(
        Path::new(BROWSER).is_file(),
        "browser test environment missing: {BROWSER}"
    );
    assert!(
        script.is_file(),
        "browser test script missing: {}",
        script.display()
    );
    let mut child = Command::new("node")
        .arg(script)
        .arg(base)
        .arg(BROWSER)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    if child
        .wait_timeout(Duration::from_secs(25))
        .unwrap()
        .is_none()
    {
        child.kill().unwrap();
        let output = child.wait_with_output().unwrap();
        panic!(
            "browser timed out: stdout: {}; stderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "browser failed: stdout: {}; stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout)
            .unwrap()
            .matches("PASS ")
            .count(),
        8
    );
}

#[tokio::test]
async fn browser_hydrates_ssr_and_renders_csr_and_static_shells() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/build-probe/tests/fixtures")
        .canonicalize()
        .unwrap();
    assert!(
        root.join("node_modules/react").is_dir(),
        "install sample packages before tests"
    );
    let generated = root.join(GENERATED_BROWSER_DIR);
    fs::create_dir_all(&generated).unwrap();
    let application = root.join("ReactApp.tsx");
    let server_path = generated.join("server.tsx");
    let framework_path = generated.join("framework.tsx");
    let client_path = generated.join("client.tsx");
    fs::write(
        &server_path,
        server_entry(application.to_str().unwrap()).unwrap(),
    )
    .unwrap();
    fs::write(&framework_path, framework_entry()).unwrap();
    let client = client_entry(application.to_str().unwrap()).unwrap();
    let observed = client.replace(
        "import {createRoot, hydrateRoot} from 'react-dom/client';",
        "import {createRoot as frameworkCreateRoot, hydrateRoot as frameworkHydrateRoot} from 'react-dom/client';\nconst createRoot = (...args) => { window.__clientRenderCall = 'createRoot'; return frameworkCreateRoot(...args); };\nconst hydrateRoot = (...args) => { window.__clientRenderCall = 'hydrateRoot'; return frameworkHydrateRoot(...args); };",
    );
    assert_ne!(observed, client);
    fs::write(&client_path, observed).unwrap();
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
    let pool = react_pool(&output);
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
    let ssr = stream_document(&adapter, &page("ssr"), &pool);
    let mut ssr_html = String::from_utf8(ssr.html).unwrap();
    ssr_html = ssr_html.replace("</div><script id=\"__SSR_PROPS__\"", "</div><script>window.__before = document.querySelector('#root main');</script><script id=\"__SSR_PROPS__\"");
    responses.insert(
        "/ssr".into(),
        ("text/html; charset=utf-8".into(), ssr_html.into_bytes()),
    );
    let empty_page = Page::from_json(br#"{"render":"ssr","title":"Example","language":"en","props":{"name":"Ada","empty":true},"state":{"count":4}}"#).unwrap();
    let empty = stream_document(&adapter, &empty_page, &pool);
    let empty_html = String::from_utf8(empty.html).unwrap();
    assert!(empty_html.contains("<div id=\"root\" data-render=\"ssr\"></div>"));
    responses.insert(
        "/empty-ssr".into(),
        ("text/html; charset=utf-8".into(), empty_html.into_bytes()),
    );
    let csr = adapter.render(&page("csr")).unwrap();
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
    let escaped_page = Page::from_json(br#"{"render":"csr","title":"<&>","language":"en","props":{"name":"</script><script>window.attack=1</script>"},"state":{"count":4}}"#).unwrap();
    let escaped = adapter.render(&escaped_page).unwrap();
    let escaped_html = String::from_utf8(escaped.html).unwrap();
    assert!(!escaped_html.contains("</script><script>window.attack=1"));
    responses.insert(
        "/escape".into(),
        ("text/html; charset=utf-8".into(), escaped_html.into_bytes()),
    );
    let shell = adapter.static_shell(&shell_page()).unwrap();
    responses.insert(
        "/shell/first".into(),
        ("text/html; charset=utf-8".into(), shell.clone()),
    );
    responses.insert(
        "/shell/second".into(),
        ("text/html; charset=utf-8".into(), shell),
    );

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
                stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
                let mut line = String::new();
                match BufReader::new(&stream).read_line(&mut line) {
                    Ok(0) => return,
                    Ok(_) => {}
                    Err(error) if matches!(error.kind(), std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock) => return,
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
    run_browser(&format!("http://{address}"), &root.join("browser.mjs"));
    let mut stop = std::net::TcpStream::connect(address).unwrap();
    stop.write_all(b"GET /__stop HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
        .unwrap();
    stop.read_exact(&mut [0_u8; 1]).unwrap();
    std::net::TcpStream::connect(address).unwrap();
    server.join().unwrap();
}

#[tokio::test]
async fn configured_dependencies_share_one_package_instance_for_provider_and_consumer() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/build-probe/tests/fixtures")
        .canonicalize()
        .unwrap();
    let root = std::env::temp_dir().join(format!("ssr-react-deps-{}", std::process::id()));
    let dependencies = root.join("dependencies");
    let nested = root.join("node_modules/shared-context");
    let configured = dependencies.join("shared-context");
    fs::create_dir_all(&nested).unwrap();
    fs::create_dir_all(&configured).unwrap();
    for name in ["react", "react-dom", "scheduler", "web-streams-polyfill"] {
        std::os::unix::fs::symlink(
            fixtures.join("node_modules").join(name),
            dependencies.join(name),
        )
        .unwrap();
    }
    let package = "import React from 'react'; export const marker = '{marker}'; export const SharedContext = React.createContext('{default}');";
    fs::write(
        nested.join("package.json"),
        r#"{"name":"shared-context","main":"index.js"}"#,
    )
    .unwrap();
    fs::write(
        configured.join("package.json"),
        r#"{"name":"shared-context","main":"index.js"}"#,
    )
    .unwrap();
    fs::write(
        nested.join("index.js"),
        package
            .replace("{marker}", "nested-copy")
            .replace("{default}", "nested-copy"),
    )
    .unwrap();
    fs::write(
        configured.join("index.js"),
        package
            .replace("{marker}", "dependency-copy")
            .replace("{default}", "missing"),
    )
    .unwrap();
    let application = root.join("application");
    fs::create_dir_all(&application).unwrap();
    fs::write(
        application.join("Consumer.jsx"),
        "import React, { useContext } from 'react'; import { SharedContext } from 'shared-context'; export function Consumer() { return <b id=\"value\">{useContext(SharedContext)}</b>; }",
    )
    .unwrap();
    fs::write(
        application.join("App.jsx"),
        "import React from 'react'; import { SharedContext } from 'shared-context'; import { Consumer } from './Consumer.jsx'; export default function App() { return <SharedContext.Provider value=\"shared-instance\"><main><Consumer /></main></SharedContext.Provider>; }",
    )
    .unwrap();
    fs::write(root.join("app.css"), "main{color:red}").unwrap();
    let root = root.canonicalize().unwrap();
    let generated = root.join("node_modules/.ssr-adapter-react-deps");
    fs::create_dir_all(&generated).unwrap();
    let server_path = generated.join("server.tsx");
    let framework_path = generated.join("framework.tsx");
    let client_path = generated.join("client.tsx");
    let application = root.join("application/App.jsx");
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
        dependencies: Some(dependencies),
    })
    .await
    .unwrap();
    let server = String::from_utf8(output.files[&output.manifest.server.path].clone()).unwrap();
    assert!(
        server.contains("missing") && !server.contains("nested-copy"),
        "the server bundle does not use the configured dependency copy alone: {server}"
    );
    let pool = react_pool(&output);
    let adapter = ReactAdapter::new(output.manifest.client.url.as_ref().unwrap(), &[]).unwrap();
    let ssr = stream_document(&adapter, &page("ssr"), &pool);
    let html = String::from_utf8(ssr.html).unwrap();
    assert!(
        html.contains("shared-instance"),
        "the provider and consumer used different package instances: {html}"
    );
    assert!(!html.contains("nested-copy"), "{html}");
    assert!(!html.contains(">missing<"), "{html}");
    fs::remove_dir_all(root).unwrap();
}
