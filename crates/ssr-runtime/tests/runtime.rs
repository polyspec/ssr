#[path = "../../ssr-build/tests/fixture/mod.rs"]
mod fixture;
mod support;
use ssr_build::{BuildConfig, build};
use ssr_core::{Page, Render};
use ssr_runtime::{Error, Pool, ServerBundle};
use std::fs;
use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, Instant};

fn page(props: &str) -> Page {
    Page::from_json(format!(r#"{{"render":"ssr","title":"T","language":"en","props":{props},"state":{{"input":3}}}}"#).as_bytes()).unwrap()
}

fn make_pool(
    entry_bytes: Vec<u8>,
    worker_count: usize,
    queue_capacity: usize,
    timeout: Duration,
) -> Result<Pool, Error> {
    Pool::new(
        ServerBundle {
            entry_path: "server/entry.js".into(),
            entry_bytes,
            chunks: Vec::new(),
        },
        support::options(worker_count, queue_capacity, timeout),
    )
}

#[test]
fn named_render_constant_is_supported() {
    let pool = make_pool(
        b"export const render = (props, state) => ({head:'', html:props.name, state});".to_vec(),
        1,
        0,
        Duration::from_secs(2),
    )
    .unwrap();
    assert_eq!(
        pool.render(&page(r#"{"name":"Ada"}"#)).unwrap().html,
        b"Ada"
    );
}

#[test]
fn promise_render_result_settles_or_reports_its_failure() {
    let fulfilled = make_pool(
        b"export function render(props, state) { return Promise.resolve().then(() => ({head:'', html: props.name, state})); }".to_vec(),
        1,
        0,
        Duration::from_secs(2),
    )
    .unwrap();
    assert_eq!(
        fulfilled.render(&page(r#"{"name":"Ada"}"#)).unwrap().html,
        b"Ada"
    );
}

#[test]
fn promise_rejection_returns_stack() {
    let rejected = make_pool(
        b"export function render() { return Promise.reject(new Error('promise failed')); }"
            .to_vec(),
        1,
        0,
        Duration::from_secs(2),
    )
    .unwrap();
    assert!(matches!(
        rejected.render(&page("{}")),
        Err(Error::JavaScript { message, stack: Some(stack) })
            if message.contains("promise failed") && stack.contains("promise failed")
    ));
}

#[test]
fn pending_promise_reports_missing_completion() {
    let pending = make_pool(
        b"export function render() { return new Promise(() => {}); }".to_vec(),
        1,
        0,
        Duration::from_secs(2),
    )
    .unwrap();
    assert!(matches!(
        pending.render(&page("{}")),
        Err(Error::InvalidResult("promise has no scheduled completion"))
    ));
}

#[test]
fn request_context_does_not_retain_globals() {
    let pool = make_pool(b"globalThis.count = 0; export function render(props, state) { globalThis.count++; return {head:'', html: String(globalThis.count), state}; }".to_vec(), 1, 1, Duration::from_secs(2)).unwrap();
    let first = pool.render(&page("{} ")).unwrap();
    let second = pool.render(&page("{} ")).unwrap();
    assert_eq!(first.html, b"1");
    assert_eq!(second.html, b"1");
    assert_eq!(first.state.compact(), r#"{"input":3}"#);
}

#[test]
fn snapshot_restores_initialized_bundle_without_reexecuting_it() {
    let source = br#"
      globalThis.initial = Array.from(crypto.getRandomValues(new Uint8Array(16))).join(',');
      export function render(props, state) {
        const html = globalThis.initial;
        globalThis.initial = 'changed';
        return {head:'', html, state};
      }
    "#;
    let pool = make_pool(source.to_vec(), 1, 0, Duration::from_secs(3)).unwrap();
    let first = pool.render(&page("{}")).unwrap();
    let second = pool.render(&page("{}")).unwrap();
    assert_eq!(first.html, second.html);
    assert_ne!(first.html, b"changed");
}

#[test]
fn snapshot_initialization_failure_is_returned() {
    assert!(matches!(
        make_pool(
            b"export function render(".to_vec(),
            1,
            0,
            Duration::from_secs(2)
        ),
        Err(Error::JavaScript { message, .. }) if message.contains("SyntaxError")
    ));
    assert!(matches!(
        make_pool(
            b"throw new Error('initialization failed')".to_vec(),
            1,
            0,
            Duration::from_secs(2),
        ),
        Err(Error::JavaScript { message, .. }) if message.contains("initialization failed")
    ));
}

#[test]
fn snapshot_initialization_obeys_pool_timeout() {
    let started = Instant::now();
    assert!(matches!(
        make_pool(b"for (;;) {}".to_vec(), 1, 0, Duration::from_millis(100)),
        Err(Error::Timeout)
    ));
    assert!(started.elapsed() < Duration::from_secs(2));
    let pool = make_pool(
        b"export function render(props, state) { return {head:'', html:'ready', state}; }".to_vec(),
        1,
        0,
        Duration::from_secs(2),
    )
    .unwrap();
    assert_eq!(pool.render(&page("{}")).unwrap().html, b"ready");
}

#[test]
fn render_metrics_report_wait_and_live_heap() {
    let pool = make_pool(
        b"export function render(props, state) { return {head:'', html:'ok', state}; }".to_vec(),
        1,
        0,
        Duration::from_secs(2),
    )
    .unwrap();
    let (result, metrics) = pool.render_with_metrics(&page("{}")).unwrap();
    assert_eq!(result.html, b"ok");
    assert!(metrics.pool_wait < Duration::from_secs(2));
    assert!(metrics.heap_used_bytes > 0);
    assert!(metrics.context_reset < Duration::from_secs(2));
    assert!(metrics.context_heap_delta_bytes > 0);
}

#[test]
fn running_script_is_terminated_and_worker_recovers() {
    let pool = make_pool(b"export function render(props, state) { if (props.loop) { for(;;) {} } return {head:'', html:'ok', state}; }".to_vec(), 1, 0, Duration::from_millis(100)).unwrap();
    assert!(matches!(
        pool.render(&page(r#"{"loop":true}"#)),
        Err(Error::Timeout)
    ));
    assert_eq!(pool.render(&page("{}")).unwrap().html, b"ok");
}

#[test]
fn full_queue_returns_explicit_error() {
    let pool = Arc::new(make_pool(b"export function render(props, state) { if (props.loop) { for(;;) {} } return {head:'', html:'ok',state}; }".to_vec(), 1, 0, Duration::from_millis(300)).unwrap());
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
fn web_apis_and_forbidden_apis() {
    let source = r#"export function render(props, state) {
      const bytes = new Uint8Array(16);
      const returned = crypto.getRandomValues(bytes);
      if (returned !== bytes || bytes.every(x => x === 0)) throw new Error('random values missing');
      let invalid;
      try { crypto.getRandomValues(new Float32Array(1)); } catch (e) { invalid = [e.name, e.code, e instanceof DOMException]; }
      let quota;
      try { crypto.getRandomValues(new Uint8Array(65537)); } catch (e) { quota = [e.name, e.code, e instanceof DOMException]; }
      let receiver;
      try { const method = crypto.getRandomValues; method(new Uint8Array(1)); } catch (e) { receiver = e.name; }
      return {head:'', html: 'ready', state: {
        invalid, quota, receiver, dom: DOMException.name, fetch: typeof fetch, timeout: typeof setTimeout,
        interval: typeof setInterval, file: typeof Deno, network: typeof WebSocket,
        randomUUID: typeof crypto.randomUUID
      }};
    }"#;
    let pool = make_pool(source.as_bytes().to_vec(), 1, 0, Duration::from_secs(2)).unwrap();
    let result = pool.render(&page("{}")).unwrap();
    assert_eq!(result.html, b"ready");
    assert_eq!(
        result.state.compact(),
        r#"{"invalid":["TypeMismatchError",17,true],"quota":["QuotaExceededError",22,true],"receiver":"TypeError","dom":"DOMException","fetch":"undefined","timeout":"undefined","interval":"undefined","file":"undefined","network":"undefined","randomUUID":"undefined"}"#
    );
}

#[test]
fn text_encoder_is_utf8_in_every_render_context() {
    let source = r#"export function render() {
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
      return {head:'', html: 'ok', state: {encoding: encoder.encoding, bytes, into, short: Array.from(short), fullResult, full: Array.from(full), receiverError, getterError, destinationError,
        fetch: typeof fetch, timeout: typeof setTimeout, file: typeof Deno}};
    }"#;
    let pool = make_pool(source.as_bytes().to_vec(), 1, 0, Duration::from_secs(2)).unwrap();
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
    let fixture = fixture::Fixture::new();
    let root = fixture.root.clone();
    let generated = root.join("generated-ssr-runtime");
    fs::create_dir_all(&generated).unwrap();
    let server_entry = generated.join("server.tsx");
    fs::write(&server_entry, "import React from 'react'; import {renderToString} from 'react-dom/server.edge'; import App from '../ReactRuntimeApp'; export const render = (props, state) => ({head:'', html: renderToString(<App {...props}/>), state});").unwrap();
    let output = build(&BuildConfig {
        root: root.clone(),
        server_entry,
        react_framework_entry: None,
        client_entry: root.join("client.tsx"),
        css_entry: root.join("app.css"),
        asset_route: "/assets".into(),
        dependencies: Some(fixture.packages.clone()),
    })
    .await
    .unwrap();
    let bundle = ServerBundle {
        entry_path: output.manifest.server.path.clone(),
        entry_bytes: output.files[&output.manifest.server.path].clone(),
        chunks: output
            .manifest
            .server_chunks
            .iter()
            .map(|file| (file.path.clone(), output.files[&file.path].clone()))
            .collect(),
    };
    let pool = Pool::new(bundle, support::options(1, 0, Duration::from_secs(10))).unwrap();
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
        let pool = make_pool(
            b"export function render(props, state) { console.log('runtime console', state.input); return {head:'', html:'ok', state}; }".to_vec(),
            1,
            0,
            Duration::from_secs(2),
        )
        .unwrap();
        assert_eq!(pool.render(&page("{}")).unwrap().html, b"ok");
        return;
    }
    // The child's standard error is read while it runs, so a full pipe cannot stop it; the
    // nextest limit of this case bounds the child, which runs in the case's process group.
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "console_writes_to_stderr", "--nocapture"])
        .env("SSR_CONSOLE_CHILD", "1")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut pipe = child.stderr.take().unwrap();
    let reader = thread::spawn(move || {
        let mut text = String::new();
        pipe.read_to_string(&mut text).map(|_| text)
    });
    let status = child.wait().unwrap();
    let stderr = reader.join().unwrap().unwrap();
    assert!(status.success(), "{stderr}");
    assert!(stderr.contains("runtime console 3"), "{stderr}");
}

#[test]
fn invalid_input_and_result_fail() {
    assert!(matches!(
        make_pool(Vec::new(), 1, 0, Duration::from_secs(1)),
        Err(Error::InvalidBundle(_))
    ));
    assert!(matches!(
        make_pool(b"x".to_vec(), 0, 0, Duration::from_secs(1)),
        Err(Error::InvalidConfiguration(_))
    ));
    let pool = make_pool(
        b"export function render() { return {head:'', html: 1, state: null}; }".to_vec(),
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
}

#[test]
fn undefined_output_state_is_rejected() {
    let pool = make_pool(
        b"export function render() { return {head:'', html: 'ok', state: {kept: 1, lost: undefined}}; }"
            .to_vec(),
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

#[test]
fn nonfinite_output_state_is_rejected() {
    let pool = make_pool(
        b"export function render() { return {head:'', html: 'ok', state: {value: NaN}}; }".to_vec(),
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

#[test]
fn server_bundle_rejects_duplicate_and_invalid_files() {
    let entry =
        b"export function render(props, state) { return {html:'ok', head:'', state}; }".to_vec();
    for bundle in [
        ServerBundle {
            entry_path: "server/entry.js".into(),
            entry_bytes: entry.clone(),
            chunks: vec![("server/entry.js".into(), b"export const value = 1".to_vec())],
        },
        ServerBundle {
            entry_path: "server/../entry.js".into(),
            entry_bytes: entry.clone(),
            chunks: Vec::new(),
        },
        ServerBundle {
            entry_path: "server/entry.js".into(),
            entry_bytes: entry,
            chunks: vec![("server/chunk.js".into(), vec![0xff])],
        },
    ] {
        assert!(matches!(
            Pool::new(bundle, support::options(1, 0, Duration::from_secs(2))),
            Err(Error::InvalidBundle(_))
        ));
    }
}

#[test]
fn render_head_is_required_and_preserved() {
    let pool = make_pool(
        b"export function render(props, state) { return {html:'<main>ok</main>', head:'<meta name=\"section\" content=\"news\">', state}; }".to_vec(),
        1,
        0,
        Duration::from_secs(2),
    ).unwrap();
    let result = pool.render(&page("{}")).unwrap();
    assert_eq!(result.head, b"<meta name=\"section\" content=\"news\">");
}

fn invalid_head(source: &str) {
    let pool = make_pool(source.as_bytes().to_vec(), 1, 0, Duration::from_secs(2)).unwrap();
    assert!(
        matches!(pool.render(&page("{}")), Err(Error::InvalidResult(_))),
        "{source}"
    );
}

#[test]
fn missing_head_is_rejected() {
    invalid_head("export function render() { return {html:'ok', state:null}; }");
}

#[test]
fn nonstring_head_is_rejected() {
    invalid_head("export function render() { return {html:'ok', head:1, state:null}; }");
}

#[test]
fn invalid_utf16_head_is_rejected() {
    invalid_head(r"export function render() { return {html:'ok', head:'\uD800', state:null}; }");
}
