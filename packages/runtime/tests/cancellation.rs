mod support;
use polyspec_ssr_core::Page;
use polyspec_ssr_runtime::{Cancellation, Pool, PoolOptions, ServerBundle};
use std::time::Duration;

const FRAMEWORK: &str = r#"
 globalThis.__ssrReact = {}; globalThis.__ssrJsxRuntime = {};
 globalThis.render = async (App, props, state) => {
   let reads = 0;
   return {state, stream: {getReader() {return {read() {
     reads++;
     if (props.loop && reads > 1) {for (;;) {}}
     if (reads > 4) return Promise.resolve({done:true});
     return Promise.resolve({done:false,value:new TextEncoder().encode('ok')});
   }}}}};
 };
"#;

fn pool() -> Pool {
    pool_with(support::options(1, 1, Duration::from_secs(10)))
}
fn pool_with(options: PoolOptions) -> Pool {
    Pool::new_react(
        ("server/framework.js".into(), FRAMEWORK.as_bytes().to_vec()),
        ServerBundle {
            entry_path: "server/app.js".into(),
            entry_bytes: b"globalThis.__ssrApp = () => 'ok';".to_vec(),
            chunks: Vec::new(),
        },
        options,
    )
    .unwrap()
}

#[test]
fn dropping_stream_cancels_running_script_and_preserves_next_request() {
    let pool = pool();
    let (_, mut stream) = pool
        .render_stream(&page(true), "nonce", Cancellation::new())
        .unwrap();
    let canceled = stream.cancel_handle();
    assert_eq!(stream.next().unwrap().unwrap(), b"ok");
    drop(stream);
    let (_, stream) = pool
        .render_stream(&page(false), "nonce", Cancellation::new())
        .unwrap();
    canceled.cancel().unwrap();
    assert_eq!(
        stream.collect::<Result<Vec<_>, _>>().unwrap().concat(),
        b"okokokok"
    );
    pool.close().unwrap();
}
fn page(looping: bool) -> Page {
    Page::from_json(format!(r#"{{"render":"ssr","title":"T","language":"en","props":{{"loop":{looping}}},"state":null}}"#).as_bytes()).unwrap()
}

#[test]
fn input_and_waiting_bytes_have_independent_limits() {
    let mut limits = support::options(1, 1, Duration::from_secs(10));
    limits.max_input_bytes = 64;
    limits.max_queue_bytes = 4;
    let pool = pool_with(limits);
    let large = Page::from_json(format!(r#"{{"render":"ssr","title":"T","language":"en","props":{{"value":"{}"}},"state":null}}"#, "x".repeat(65)).as_bytes()).unwrap();
    assert!(matches!(
        pool.render_stream(&large, "nonce", Cancellation::new()),
        Err(polyspec_ssr_runtime::Error::LimitExceeded("input bytes"))
    ));
    let (_, mut stream) = pool
        .render_stream(&page(false), "nonce", Cancellation::new())
        .unwrap();
    assert!(matches!(
        pool.render_stream(&page(false), "nonce", Cancellation::new()),
        Err(polyspec_ssr_runtime::Error::LimitExceeded("queue bytes"))
    ));
    stream.close().unwrap();
    pool.health().unwrap();
}

#[test]
fn stream_chunk_limit_splits_output_without_truncation() {
    let mut limits = support::options(1, 1, Duration::from_secs(10));
    limits.max_chunk_bytes = 1;
    let pool = pool_with(limits);
    let (_, stream) = pool
        .render_stream(&page(false), "nonce", Cancellation::new())
        .unwrap();
    let chunks = stream.collect::<Result<Vec<_>, _>>().unwrap();
    assert!(chunks.iter().all(|chunk| chunk.len() <= 1));
    assert_eq!(chunks.concat(), b"okokokok");
    pool.health().unwrap();
}

#[test]
fn stream_output_limit_counts_state_and_all_chunks() {
    let mut limits = support::options(1, 1, Duration::from_secs(10));
    limits.max_output_bytes = 7;
    let pool = pool_with(limits);
    let (_, mut stream) = pool
        .render_stream(&page(false), "nonce", Cancellation::new())
        .unwrap();
    assert_eq!(stream.next().unwrap().unwrap(), b"ok");
    assert!(matches!(
        stream.next(),
        Some(Err(polyspec_ssr_runtime::Error::LimitExceeded(
            "output bytes"
        )))
    ));
    pool.health().unwrap();
}
