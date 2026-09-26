use ssr_adapter_react::{ReactAdapter, client_entry, server_entry};
use ssr_build::{BuildConfig, build};
use ssr_core::Page;
use ssr_runtime::Pool;
use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;
use wait_timeout::ChildExt;

fn page(render: &str) -> Page {
    Page::from_json(format!(r#"{{"render":"{render}","title":"Example","language":"en","props":{{"name":"Ada"}},"state":{{"count":4}}}}"#).as_bytes()).unwrap()
}

fn shell_page() -> Page {
    Page::from_json(
        br#"{"render":"csr","title":"Example","language":"en","props":{},"state":null}"#,
    )
    .unwrap()
}

#[test]
fn server_result_and_csr_state_follow_the_same_client_build() {
    let server = server_entry("/application/App.tsx").unwrap();
    let client = client_entry("/application/App.tsx").unwrap();
    assert!(server.contains("renderToString"));
    assert!(client.contains("hydrateRoot"));
    assert!(client.contains("createRoot"));

    let pool = Pool::new(b"function render(props, state) { return {html: '<main>' + props.name + '</main>', state: {count: state.count + 1}}; }".to_vec(), 1, 0, Duration::from_secs(2)).unwrap();
    let adapter = ReactAdapter::new("/assets/client.js", &["/assets/style.css"]).unwrap();
    let ssr = adapter.render(&page("ssr"), &pool).unwrap();
    assert_eq!(ssr.state.compact(), r#"{"count":5}"#);
    let ssr_html = String::from_utf8(ssr.html).unwrap();
    assert!(ssr_html.contains("<main>Ada</main>"));
    assert!(ssr_html.contains("/assets/client.js"));
    assert!(ssr_html.contains(r#"{"count":5}"#));

    let csr = adapter.render(&page("csr"), &pool).unwrap();
    assert_eq!(csr.state.compact(), r#"{"count":4}"#);
    let csr_html = String::from_utf8(csr.html).unwrap();
    assert!(csr_html.contains("<div id=\"root\"></div>"));
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
    let generated = root.join("node_modules/.ssr-adapter-react");
    fs::create_dir_all(&generated).unwrap();
    let application = root.join("ReactApp.tsx");
    let server_path = generated.join("server.tsx");
    let client_path = generated.join("client.tsx");
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
        client_entry: client_path,
        css_entry: root.join("app.css"),
        asset_route: "/assets".into(),
    })
    .await
    .unwrap();
    let server = output.files[&output.manifest.server.path].clone();
    let pool = Pool::new(server, 1, 0, Duration::from_secs(10)).unwrap();
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
    let ssr = adapter.render(&page("ssr"), &pool).unwrap();
    assert_eq!(ssr.state.compact(), r#"{"count":5}"#);
    let html = String::from_utf8(ssr.html).unwrap();
    assert!(html.contains("<h1>Ada</h1>"), "{html}");
    let csr = adapter.render(&page("csr"), &pool).unwrap();
    assert_eq!(csr.state.compact(), r#"{"count":4}"#);
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
        5
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
    let generated = root.join("node_modules/.ssr-adapter-react");
    fs::create_dir_all(&generated).unwrap();
    let application = root.join("ReactApp.tsx");
    let server_path = generated.join("server.tsx");
    let client_path = generated.join("client.tsx");
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
        client_entry: client_path,
        css_entry: root.join("app.css"),
        asset_route: "/assets".into(),
    })
    .await
    .unwrap();
    let pool = Pool::new(
        output.files[&output.manifest.server.path].clone(),
        1,
        0,
        Duration::from_secs(10),
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
    let mut ssr_html = String::from_utf8(ssr.html).unwrap();
    ssr_html = ssr_html.replace("</div><script id=\"__SSR_PROPS__\"", "</div><script>window.__before = document.querySelector('#root main');</script><script id=\"__SSR_PROPS__\"");
    responses.insert(
        "/ssr".into(),
        ("text/html; charset=utf-8".into(), ssr_html.into_bytes()),
    );
    let csr = adapter.render(&page("csr"), &pool).unwrap();
    responses.insert("/csr".into(), ("text/html; charset=utf-8".into(), csr.html));
    let escaped_page = Page::from_json(br#"{"render":"csr","title":"<&>","language":"en","props":{"name":"</script><script>window.attack=1</script>"},"state":{"count":4}}"#).unwrap();
    let escaped = adapter.render(&escaped_page, &pool).unwrap();
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
