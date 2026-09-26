use ssr_adapter_react::{ReactAdapter, client_entry, framework_entry, server_entry};
use ssr_build::{BuildConfig, build};
use ssr_core::Page;
use ssr_runtime::{Pool, ServerBundle};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;
use wait_timeout::ChildExt;

#[cfg(target_os = "macos")]
const BROWSER: &str = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
#[cfg(target_os = "linux")]
const BROWSER: &str = "/usr/bin/google-chrome";

fn document(adapter: &ReactAdapter, pool: &Pool, fail_client: bool) -> Vec<u8> {
    let json = format!(
        r#"{{"render":"ssr","title":"Stream","language":"en","props":{{"failClient":{fail_client}}},"state":{{"count":4}}}}"#
    );
    let page = Page::from_json(json.as_bytes()).unwrap();
    let (state, stream) = pool.render_stream(&page, "browser_nonce").unwrap();
    let (mut prefix, suffix) = adapter.stream_parts(&page, &state).unwrap();
    let body = stream.collect::<Result<Vec<_>, _>>().unwrap().concat();
    let text = std::str::from_utf8(&body).unwrap();
    assert!(text.contains("id=\"fallback\""), "{text}");
    assert!(text.contains("<!--$!--><template data-msg="), "{text}");
    prefix.extend_from_slice(&body);
    prefix.extend_from_slice(&suffix);
    prefix
}

fn browser(base: &str, script: &Path) {
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
        2
    );
}

#[tokio::test]
async fn browser_retries_failed_suspense_content_and_uses_error_boundary() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/build-probe/tests/fixtures")
        .canonicalize()
        .unwrap();
    assert!(
        root.join("node_modules/react").is_dir(),
        "sample packages are required"
    );
    let generated = root.join("node_modules/.ssr-adapter-react-recovery");
    std::fs::create_dir_all(&generated).unwrap();
    let application = root.join("ReactStreamApp.tsx");
    let server_path = generated.join("server.tsx");
    let framework_path = generated.join("framework.tsx");
    let client_path = generated.join("client.tsx");
    std::fs::write(
        &server_path,
        server_entry(application.to_str().unwrap()).unwrap(),
    )
    .unwrap();
    std::fs::write(&framework_path, framework_entry()).unwrap();
    std::fs::write(
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
    })
    .await
    .unwrap();
    assert!(output.manifest.server_chunks.is_empty());
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
    responses.insert(
        "/recover".into(),
        (
            "text/html; charset=utf-8".into(),
            document(&adapter, &pool, false),
        ),
    );
    responses.insert(
        "/client-error".into(),
        (
            "text/html; charset=utf-8".into(),
            document(&adapter, &pool, true),
        ),
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
    browser(
        &format!("http://{address}"),
        &root.join("browser-react-stream.mjs"),
    );
    let mut stop = std::net::TcpStream::connect(address).unwrap();
    stop.write_all(b"GET /__stop HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
        .unwrap();
    stop.read_exact(&mut [0_u8; 1]).unwrap();
    std::net::TcpStream::connect(address).unwrap();
    server.join().unwrap();
}
