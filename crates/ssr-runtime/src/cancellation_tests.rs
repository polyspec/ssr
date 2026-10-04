use crate::{Cancellation, Pool, PoolOptions, ServerBundle, pool::QueueEntry};
use ssr_core::Page;
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
    pool_with(options())
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
fn completed_cancellation_does_not_terminate_the_next_request() {
    let pool = pool();
    for _ in 0..20 {
        let (_, stream) = pool
            .render_stream(&page(false), "nonce", Cancellation::new())
            .unwrap();
        let completed = stream.cancel_handle();
        assert_eq!(
            stream.collect::<Result<Vec<_>, _>>().unwrap().concat(),
            b"okokokok"
        );
        let (_, stream) = pool
            .render_stream(&page(false), "nonce", Cancellation::new())
            .unwrap();
        completed.cancel().unwrap();
        assert_eq!(
            stream.collect::<Result<Vec<_>, _>>().unwrap().concat(),
            b"okokokok"
        );
    }
}

#[test]
fn cancel_handle_interrupts_running_script_without_a_reader() {
    let pool = pool();
    let (_, mut stream) = pool
        .render_stream(&page(true), "nonce", Cancellation::new())
        .unwrap();
    let cancel = stream.cancel_handle();
    assert_eq!(stream.next().unwrap().unwrap(), b"ok");
    cancel.cancel().unwrap();
    assert!(matches!(stream.next(), Some(Err(crate::Error::Canceled))));
    let (_, stream) = pool
        .render_stream(&page(false), "nonce", Cancellation::new())
        .unwrap();
    assert_eq!(
        stream.collect::<Result<Vec<_>, _>>().unwrap().concat(),
        b"okokokok"
    );
}

#[test]
fn close_interrupts_running_script_joins_threads_and_rejects_calls() {
    let pool = pool();
    let (_, mut stream) = pool
        .render_stream(&page(true), "nonce", Cancellation::new())
        .unwrap();
    assert_eq!(stream.next().unwrap().unwrap(), b"ok");
    pool.close().unwrap();
    assert!(matches!(pool.health(), Err(crate::Error::WorkerStopped)));
    assert!(matches!(
        stream.next(),
        Some(Err(crate::Error::WorkerStopped))
    ));
    assert!(matches!(
        pool.render_stream(&page(false), "nonce", Cancellation::new()),
        Err(crate::Error::WorkerStopped)
    ));
    pool.close().unwrap();
}

#[test]
fn canceled_input_and_reused_cancellation_return_explicit_errors() {
    let pool = pool();
    let cancel = Cancellation::new();
    cancel.cancel().unwrap();
    assert!(matches!(
        pool.render_stream(&page(false), "nonce", cancel),
        Err(crate::Error::Canceled)
    ));
    let cancel = Cancellation::new();
    let (_, stream) = pool
        .render_stream(&page(false), "nonce", cancel.clone())
        .unwrap();
    stream.collect::<Result<Vec<_>, _>>().unwrap();
    assert!(matches!(
        pool.render_stream(&page(false), "nonce", cancel),
        Err(crate::Error::InvalidConfiguration(_))
    ));
}

#[test]
fn cancellation_wakes_a_queued_call_before_worker_release() {
    let mut limits = options();
    limits.timeout = Duration::from_secs(10);
    let pool = std::sync::Arc::new(pool_with(limits));
    let (_, mut active) = pool
        .render_stream(&page(false), "nonce", Cancellation::new())
        .unwrap();
    let entered = enter_queue_after(&pool, Duration::from_millis(300));
    let cancel = Cancellation::new();
    let waiting_cancel = cancel.clone();
    let waiting_pool = pool.clone();
    let thread = std::thread::spawn(move || {
        waiting_pool.render_stream(&page(false), "nonce", waiting_cancel)
    });
    entered.recv().unwrap();
    cancel.cancel().unwrap();
    assert!(matches!(
        thread.join().unwrap(),
        Err(crate::Error::Canceled)
    ));
    active.close().unwrap();
    pool.health().unwrap();
}

#[test]
fn timeout_covers_queue_wait_and_execution() {
    let mut limits = options();
    limits.timeout = Duration::from_secs(2);
    let pool = std::sync::Arc::new(pool_with(limits));
    let (_, mut active) = pool
        .render_stream(&page(false), "nonce", Cancellation::new())
        .unwrap();
    let entered = enter_queue_after(&pool, Duration::from_millis(1100));
    let waiting_pool = pool.clone();
    let waiting = std::thread::spawn(move || {
        let (_, stream) = waiting_pool.render_stream(&page(true), "nonce", Cancellation::new())?;
        stream.collect::<Result<Vec<_>, _>>()
    });
    entered.recv().unwrap();
    std::thread::sleep(Duration::from_millis(600));
    active.close().unwrap();
    assert!(matches!(
        waiting.join().unwrap(),
        Err(crate::Error::Timeout)
    ));
    pool.health().unwrap();
}

fn enter_queue_after(pool: &Pool, delay: Duration) -> crossbeam_channel::Receiver<()> {
    let (entered, received) = crossbeam_channel::bounded(1);
    *pool.queue_entry.lock().unwrap() = QueueEntry {
        delay,
        entered: Some(entered),
    };
    received
}
fn page(looping: bool) -> Page {
    Page::from_json(format!(r#"{{"render":"ssr","title":"T","language":"en","props":{{"loop":{looping}}},"state":null}}"#).as_bytes()).unwrap()
}

#[test]
fn dropping_stream_interrupts_javascript_and_releases_worker() {
    let pool = pool();
    let (_, mut stream) = pool
        .render_stream(&page(true), "nonce", Cancellation::new())
        .unwrap();
    assert_eq!(stream.next().unwrap().unwrap(), b"ok");
    drop(stream);
    let (_, stream) = pool
        .render_stream(&page(false), "nonce", Cancellation::new())
        .unwrap();
    assert_eq!(
        stream.collect::<Result<Vec<_>, _>>().unwrap().concat(),
        b"okokokok"
    );
}

#[test]
fn unread_stream_expires_and_releases_worker() {
    let pool = pool();
    let (_, stream) = pool
        .render_stream(&page(false), "nonce", Cancellation::new())
        .unwrap();
    std::thread::sleep(Duration::from_millis(400));
    let (_, next) = pool
        .render_stream(&page(false), "nonce", Cancellation::new())
        .unwrap();
    assert_eq!(
        next.collect::<Result<Vec<_>, _>>().unwrap().concat(),
        b"okokokok"
    );
    drop(stream);
}

fn options() -> PoolOptions {
    PoolOptions {
        worker_count: 1,
        queue_capacity: 1,
        timeout: Duration::from_millis(300),
        cleanup_timeout: Duration::from_millis(500),
        max_input_bytes: 1048576,
        max_queue_bytes: 2097152,
        max_chunk_bytes: 65536,
        max_output_bytes: 4194304,
        max_heap_bytes: 134217728,
    }
}

#[test]
fn pool_closure_notifies_health_waiter_without_polling() {
    let pool = std::sync::Arc::new(pool());
    let waiting = pool.clone();
    let (sent, received) = crossbeam_channel::bounded(1);
    let waiter = std::thread::spawn(move || sent.send(waiting.wait()).unwrap());
    pool.close().unwrap();
    assert!(matches!(
        received.recv_timeout(Duration::from_secs(1)).unwrap(),
        Err(crate::Error::WorkerStopped)
    ));
    waiter.join().unwrap();
}

#[test]
fn explicit_limits_reject_zero_and_clock_overflow() {
    let valid = options();
    for invalid in [
        PoolOptions {
            worker_count: 0,
            ..valid
        },
        PoolOptions {
            timeout: Duration::ZERO,
            ..valid
        },
        PoolOptions {
            cleanup_timeout: Duration::ZERO,
            ..valid
        },
        PoolOptions {
            timeout: Duration::MAX,
            ..valid
        },
        PoolOptions {
            max_input_bytes: 0,
            ..valid
        },
        PoolOptions {
            max_queue_bytes: 0,
            ..valid
        },
        PoolOptions {
            max_chunk_bytes: 0,
            ..valid
        },
        PoolOptions {
            max_output_bytes: 0,
            ..valid
        },
        PoolOptions {
            max_heap_bytes: 0,
            ..valid
        },
    ] {
        assert!(matches!(
            invalid.validate(),
            Err(crate::Error::InvalidConfiguration(_))
        ));
    }
    PoolOptions {
        queue_capacity: 0,
        max_queue_bytes: 0,
        ..valid
    }
    .validate()
    .unwrap();
}
