use ssr_build::{BuildConfig, build};
use ssr_core::{Page, Render};
use ssr_runtime::{Error, Pool};
use std::fs;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, Instant};
use wait_timeout::ChildExt;

fn page(props: &str) -> Page {
    Page::from_json(format!(r#"{{"render":"ssr","title":"T","language":"en","props":{props},"state":{{"input":3}}}}"#).as_bytes()).unwrap()
}

#[test]
fn request_context_does_not_retain_globals() {
    let pool = Pool::new(b"globalThis.count = 0; function render(props, state) { globalThis.count++; return {html: String(globalThis.count), state}; }".to_vec(), 1, 1, Duration::from_secs(2)).unwrap();
    let first = pool.render(&page("{} ")).unwrap();
    let second = pool.render(&page("{} ")).unwrap();
    assert_eq!(first.html, b"1");
    assert_eq!(second.html, b"1");
    assert_eq!(first.state.compact(), r#"{"input":3}"#);
}

#[test]
fn render_metrics_report_wait_and_live_heap() {
    let pool = Pool::new(
        b"function render(props, state) { return {html:'ok', state}; }".to_vec(),
        1,
        0,
        Duration::from_secs(2),
    )
    .unwrap();
    let (result, metrics) = pool.render_with_metrics(&page("{}")).unwrap();
    assert_eq!(result.html, b"ok");
    assert!(metrics.pool_wait < Duration::from_secs(2));
    assert!(metrics.heap_used_bytes > 0);
}

#[test]
fn running_script_is_terminated_and_worker_recovers() {
    let pool = Pool::new(b"function render(props, state) { if (props.loop) { for(;;) {} } return {html:'ok', state}; }".to_vec(), 1, 0, Duration::from_millis(100)).unwrap();
    assert!(matches!(
        pool.render(&page(r#"{"loop":true}"#)),
        Err(Error::Timeout)
    ));
    assert_eq!(pool.render(&page("{}")).unwrap().html, b"ok");
}

#[test]
fn full_queue_returns_explicit_error() {
    let pool = Arc::new(Pool::new(b"function render(props, state) { if (props.loop) { for(;;) {} } return {html:'ok',state}; }".to_vec(), 1, 0, Duration::from_millis(300)).unwrap());
    let barrier = Arc::new(Barrier::new(3));
    let handles: Vec<_> = (0..2)
        .map(|_| {
            let pool = pool.clone();
            let barrier = barrier.clone();
            thread::spawn(move || {
                barrier.wait();
                pool.render(&page(r#"{"loop":true}"#))
            })
        })
        .collect();
    barrier.wait();
    let results: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect();
    assert!(
        results
            .iter()
            .any(|result| matches!(result, Err(Error::QueueFull))),
        "{results:?}"
    );
    assert!(
        results
            .iter()
            .any(|result| matches!(result, Err(Error::Timeout))),
        "{results:?}"
    );
}

#[test]
fn timeout_covers_queue_wait_and_execution() {
    let pool = Arc::new(
        Pool::new(
            b"function render(props, state) { const until = Date.now() + 150; while (Date.now() < until) {} return {html:'ok', state}; }".to_vec(),
            1,
            1,
            Duration::from_millis(250),
        )
        .unwrap(),
    );
    let first_pool = Arc::clone(&pool);
    let (started, ready) = std::sync::mpsc::sync_channel(0);
    let first = thread::spawn(move || {
        started.send(()).unwrap();
        first_pool.render(&page("{}"))
    });
    ready.recv().unwrap();
    thread::sleep(Duration::from_millis(30));
    let start = Instant::now();
    assert!(matches!(pool.render(&page("{}")), Err(Error::Timeout)));
    assert!(start.elapsed() < Duration::from_millis(350));
    assert_eq!(first.join().unwrap().unwrap().html, b"ok");
}

#[test]
fn web_apis_and_forbidden_apis() {
    let source = r#"function render(props, state) {
      const bytes = new Uint8Array(16);
      const returned = crypto.getRandomValues(bytes);
      if (returned !== bytes || bytes.every(x => x === 0)) throw new Error('random values missing');
      let invalid;
      try { crypto.getRandomValues(new Float32Array(1)); } catch (e) { invalid = [e.name, e.code, e instanceof DOMException]; }
      let quota;
      try { crypto.getRandomValues(new Uint8Array(65537)); } catch (e) { quota = [e.name, e.code, e instanceof DOMException]; }
      let receiver;
      try { const method = crypto.getRandomValues; method(new Uint8Array(1)); } catch (e) { receiver = e.name; }
      return {html: 'ready', state: {
        invalid, quota, receiver, dom: DOMException.name, fetch: typeof fetch, timeout: typeof setTimeout,
        interval: typeof setInterval, file: typeof Deno, network: typeof WebSocket,
        randomUUID: typeof crypto.randomUUID
      }};
    }"#;
    let pool = Pool::new(source.as_bytes().to_vec(), 1, 0, Duration::from_secs(2)).unwrap();
    let result = pool.render(&page("{}")).unwrap();
    assert_eq!(result.html, b"ready");
    assert_eq!(
        result.state.compact(),
        r#"{"invalid":["TypeMismatchError",17,true],"quota":["QuotaExceededError",22,true],"receiver":"TypeError","dom":"DOMException","fetch":"undefined","timeout":"undefined","interval":"undefined","file":"undefined","network":"undefined","randomUUID":"undefined"}"#
    );
}

#[test]
fn text_encoder_is_utf8_in_every_render_context() {
    let source = r#"function render() {
      const encoder = new TextEncoder();
      const bytes = Array.from(encoder.encode('Aé😀\uD800'));
      const short = new Uint8Array(4);
      const into = encoder.encodeInto('Aé😀', short);
      const full = new Uint8Array(10);
      const fullResult = encoder.encodeInto('Aé😀\uD800', full);
      let receiverError = false;
      let getterError = false;
      let destinationError = false;
      try { TextEncoder.prototype.encode.call({}); } catch (error) { receiverError = error instanceof TypeError; }
      try { Object.getOwnPropertyDescriptor(TextEncoder.prototype, 'encoding').get.call({}); } catch (error) { getterError = error instanceof TypeError; }
      try { encoder.encodeInto('x', new Uint16Array(2)); } catch (error) { destinationError = error instanceof TypeError; }
      return {html: 'ok', state: {encoding: encoder.encoding, bytes, into, short: Array.from(short), fullResult, full: Array.from(full), receiverError, getterError, destinationError,
        fetch: typeof fetch, timeout: typeof setTimeout, file: typeof Deno}};
    }"#;
    let pool = Pool::new(source.as_bytes().to_vec(), 1, 0, Duration::from_secs(2)).unwrap();
    for _ in 0..2 {
        let result = pool.render(&page("{}")).unwrap();
        assert_eq!(result.html, b"ok");
        assert_eq!(
            result.state.compact(),
            r#"{"encoding":"utf-8","bytes":[65,195,169,240,159,152,128,239,191,189],"into":{"read":2,"written":3},"short":[65,195,169,0],"fullResult":{"read":5,"written":10},"full":[65,195,169,240,159,152,128,239,191,189],"receiverError":true,"getterError":true,"destinationError":true,"fetch":"undefined","timeout":"undefined","file":"undefined"}"#
        );
    }
}

#[tokio::test]
async fn react_server_bundle_executes_with_text_encoder() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/build-probe/tests/fixtures")
        .canonicalize()
        .unwrap();
    assert!(
        root.join("node_modules/react").is_dir(),
        "install sample packages before tests"
    );
    let generated = root.join("node_modules/.ssr-runtime");
    fs::create_dir_all(&generated).unwrap();
    let server_entry = generated.join("server.tsx");
    fs::write(&server_entry, "import React from 'react'; import {renderToString} from 'react-dom/server.edge'; import App from '../../ReactRuntimeApp'; globalThis.render = (props, state) => ({html: renderToString(<App {...props}/>), state});").unwrap();
    let output = build(&BuildConfig {
        root: root.clone(),
        server_entry,
        client_entry: root.join("client.tsx"),
        css_entry: root.join("app.css"),
        asset_route: "/assets".into(),
    })
    .await
    .unwrap();
    let bundle = output.files[&output.manifest.server.path].clone();
    let pool = Pool::new(bundle, 1, 0, Duration::from_secs(10)).unwrap();
    let request = Page::from_json(br#"{"render":"ssr","title":"T","language":"en","props":{"name":"Ada"},"state":{"count":4}}"#).unwrap();
    let result = pool.render(&request).unwrap();
    assert!(
        String::from_utf8(result.html)
            .unwrap()
            .contains("<h1>Ada</h1>")
    );
    assert_eq!(result.state.compact(), r#"{"count":4}"#);
}

#[test]
fn console_writes_to_stderr() {
    if std::env::var_os("SSR_CONSOLE_CHILD").is_some() {
        let pool = Pool::new(
            b"function render(props, state) { console.log('runtime console', state.input); return {html:'ok', state}; }".to_vec(),
            1,
            0,
            Duration::from_secs(2),
        )
        .unwrap();
        assert_eq!(pool.render(&page("{}")).unwrap().html, b"ok");
        return;
    }
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "console_writes_to_stderr", "--nocapture"])
        .env("SSR_CONSOLE_CHILD", "1")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let status = match child.wait_timeout(Duration::from_secs(5)).unwrap() {
        Some(status) => status,
        None => {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("console subprocess timed out");
        }
    };
    let mut stderr = String::new();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();
    assert!(status.success(), "{stderr}");
    assert!(stderr.contains("runtime console 3"), "{stderr}");
}

#[test]
fn invalid_input_and_result_fail() {
    assert!(matches!(
        Pool::new(Vec::new(), 1, 0, Duration::from_secs(1)),
        Err(Error::InvalidBundle(_))
    ));
    assert!(matches!(
        Pool::new(b"x".to_vec(), 0, 0, Duration::from_secs(1)),
        Err(Error::InvalidConfiguration(_))
    ));
    let pool = Pool::new(
        b"function render() { return {html: 1, state: null}; }".to_vec(),
        1,
        0,
        Duration::from_secs(2),
    )
    .unwrap();
    assert!(matches!(
        pool.render(&page("{}")),
        Err(Error::InvalidResult(_))
    ));
    let mut csr = page("{}");
    csr.render = Render::Csr;
    assert!(matches!(pool.render(&csr), Err(Error::InvalidPage(_))));
    let pool = Pool::new(
        b"function render() { return {html: 'ok', state: {kept: 1, lost: undefined}}; }".to_vec(),
        1,
        0,
        Duration::from_secs(2),
    )
    .unwrap();
    assert!(matches!(
        pool.render(&page("{}")),
        Err(Error::JavaScript { .. })
    ));
    let pool = Pool::new(
        b"function render() { return {html: 'ok', state: {value: NaN}}; }".to_vec(),
        1,
        0,
        Duration::from_secs(2),
    )
    .unwrap();
    assert!(matches!(
        pool.render(&page("{}")),
        Err(Error::JavaScript { .. })
    ));
}
