mod support;
use ssr_core::Page;
use ssr_runtime::{Error, Pool, ServerBundle};
use std::sync::{Arc, Barrier};
use std::time::Duration;

fn pool(source: &[u8]) -> Result<Pool, Error> {
    Pool::new(
        ServerBundle {
            entry_path: "server/entry.js".into(),
            entry_bytes: source.to_vec(),
            chunks: Vec::new(),
        },
        support::options(4, 16, Duration::from_secs(5)),
    )
}

#[test]
fn one_process_rejects_another_snapshot_key_after_all_pools_close() {
    const SOURCE: &[u8] =
        b"export function render(props, state) { return {head:'', html:'first', state}; }";
    let first = pool(SOURCE).unwrap();
    let same = pool(SOURCE).unwrap();
    drop(first);
    drop(same);
    let different =
        pool(b"export function render(props, state) { return {head:'', html:'second', state}; }");
    assert!(matches!(
        different,
        Err(Error::Snapshot(
            "renderer process already selected another snapshot key"
        ))
    ));
}

#[test]
fn same_snapshot_restores_parallel_workers_without_initialization_or_state_leaks() {
    let pool = Arc::new(
        pool(
            br#"
        globalThis.initial = Array.from(crypto.getRandomValues(new Uint8Array(16))).join(',');
        globalThis.count = 0;
        export function render(props, state) {
            const html = globalThis.initial + ':' + ++globalThis.count;
            globalThis.initial = 'changed';
            return {head:'', html, state};
        }
    "#,
        )
        .unwrap(),
    );
    let barrier = Arc::new(Barrier::new(4));
    let results = std::thread::scope(|threads| {
        let workers: Vec<_> = (0..4).map(|_| {
            let pool = Arc::clone(&pool);
            let barrier = Arc::clone(&barrier);
            threads.spawn(move || {
                barrier.wait();
                (0..20).map(|_| {
                    let page = Page::from_json(br#"{"render":"ssr","title":"T","language":"en","props":{},"state":null}"#).unwrap();
                    pool.render(&page).unwrap().html
                }).collect::<Vec<_>>()
            })
        }).collect();
        workers
            .into_iter()
            .flat_map(|worker| worker.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(results.len(), 80);
    for result in &results {
        assert_eq!(result, &results[0]);
        assert!(result.ends_with(b":1"));
        assert!(!result.starts_with(b"changed"));
    }
}

#[tokio::test]
async fn svelte_compilation_after_snapshot_selection_returns_an_error() {
    use ssr_build::{BuildConfig, build};
    use std::path::Path;
    use std::process::Command;
    use wait_timeout::ChildExt;
    const TEST: &str = "svelte_compilation_after_snapshot_selection_returns_an_error";
    if std::env::var_os("SSR_COMPILER_AFTER_SNAPSHOT").is_some() {
        let _pool =
            pool(b"export function render(props, state) { return {head:'', html:'ok', state}; }")
                .unwrap();
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tools/build-probe/tests/fixtures")
            .canonicalize()
            .unwrap();
        let generated = root.join("node_modules/.ssr-runtime-process");
        std::fs::create_dir_all(&generated).unwrap();
        let server = generated.join("server.js");
        let client = generated.join("client.js");
        let source = "import App from '../../SvelteApp.svelte'; export const render = () => App;";
        std::fs::write(&server, source).unwrap();
        std::fs::write(&client, source).unwrap();
        let result = build(&BuildConfig {
            root: root.clone(),
            server_entry: server,
            client_entry: client,
            react_framework_entry: None,
            css_entry: root.join("app.css"),
            asset_route: "/assets".into(),
        })
        .await;
        let diagnostic = result
            .as_ref()
            .map(|_| "compiled")
            .map_err(ToString::to_string);
        assert!(
            matches!(&result, Err(ssr_build::Error::JavaScript(message)) if message.contains("Svelte compilation cannot run after renderer initialization")),
            "compiler result: {diagnostic:?}"
        );
        return;
    }
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", TEST, "--nocapture"])
        .env("SSR_COMPILER_AFTER_SNAPSHOT", "1")
        .spawn()
        .unwrap();
    match child.wait_timeout(Duration::from_secs(20)).unwrap() {
        Some(status) => assert!(
            status.success(),
            "compiler process did not return the required error: {status}"
        ),
        None => {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("compiler process timed out");
        }
    }
}

#[tokio::test]
async fn failed_snapshot_disposes_its_creator_before_compilation_and_render_retry() {
    use ssr_build::{BuildConfig, build};
    use std::path::Path;
    let failed =
        pool(b"throw new Error('snapshot initialization failed'); export function render() {}");
    assert!(
        matches!(failed, Err(Error::JavaScript { message, .. }) if message.contains("snapshot initialization failed"))
    );
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/build-probe/tests/fixtures")
        .canonicalize()
        .unwrap();
    let generated = root.join("node_modules/.ssr-runtime-failed-snapshot");
    std::fs::create_dir_all(&generated).unwrap();
    let server = generated.join("server.js");
    let client = generated.join("client.js");
    let source = "import App from '../../SvelteApp.svelte'; export const render = () => App;";
    std::fs::write(&server, source).unwrap();
    std::fs::write(&client, source).unwrap();
    let compiled = build(&BuildConfig {
        root: root.clone(),
        server_entry: server,
        client_entry: client,
        react_framework_entry: None,
        css_entry: root.join("app.css"),
        asset_route: "/assets".into(),
    })
    .await
    .unwrap();
    assert!(!compiled.files.is_empty());
    let renderer =
        pool(b"export function render(props, state) { return {head:'', html:'retried', state}; }")
            .unwrap();
    let page =
        Page::from_json(br#"{"render":"ssr","title":"T","language":"en","props":{},"state":null}"#)
            .unwrap();
    assert_eq!(renderer.render(&page).unwrap().html, b"retried");
}
