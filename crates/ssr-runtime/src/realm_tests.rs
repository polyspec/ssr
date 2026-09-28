use crate::{Pool, ServerBundle, web};
use deno_core::v8;
use sha2::{Digest, Sha256};
use ssr_build::{BuildConfig, build};
use ssr_core::Page;
use std::path::Path;
use std::time::Duration;

fn execute<'s>(scope: &mut v8::PinScope<'s, '_>, source: &str) -> v8::Local<'s, v8::Value> {
    let source = v8::String::new(scope, source).unwrap();
    let script = v8::Script::compile(scope, source, None).unwrap();
    script.run(scope).unwrap()
}

#[tokio::test]
async fn react_initialization_and_hooks_share_context_without_application_timers() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/build-probe/tests/fixtures")
        .canonicalize()
        .unwrap();
    assert!(root.join("node_modules/react").is_dir());
    let output = build(&BuildConfig {
        root: root.clone(),
        server_entry: root.join("realm-application.tsx"),
        react_framework_entry: Some(root.join("realm-framework.tsx")),
        client_entry: root.join("client.tsx"),
        css_entry: root.join("app.css"),
        asset_route: "/assets".into(),
    })
    .await
    .unwrap();
    let framework_file = output.manifest.react_framework.as_ref().unwrap();
    assert!(framework_file.url.is_none());
    let framework_bytes = &output.files[&framework_file.path];
    assert_eq!(
        framework_file.sha256,
        Sha256::digest(framework_bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    assert_eq!(output.manifest.source_maps.len(), 2);
    let framework = std::str::from_utf8(framework_bytes).unwrap();
    let application = std::str::from_utf8(&output.files[&output.manifest.server.path]).unwrap();

    deno_core::JsRuntime::init_platform(None);
    let mut isolate = v8::Isolate::new(Default::default());
    v8::scope!(let scope, &mut isolate);
    let context = v8::Context::new(scope, Default::default());
    let scope = &mut v8::ContextScope::new(scope, context);
    web::install(scope, context).unwrap();
    execute(scope, "globalThis.setTimeout = function () {};");
    let timer_name = v8::String::new(scope, "setTimeout").unwrap();
    let timer = context.global(scope).get(scope, timer_name.into()).unwrap();
    let source = v8::String::new(scope, framework).unwrap();
    let mut source = v8::script_compiler::Source::new(source, None);
    let function = v8::script_compiler::compile_function(
        scope,
        &mut source,
        &[timer_name],
        &[],
        v8::script_compiler::CompileOptions::NoCompileOptions,
        v8::script_compiler::NoCacheReason::NoReason,
    )
    .unwrap();
    function
        .call(scope, context.global(scope).into(), &[timer])
        .unwrap();
    assert_eq!(
        context.global(scope).delete(scope, timer_name.into()),
        Some(true)
    );
    assert_eq!(
        execute(scope, "typeof setTimeout").to_rust_string_lossy(scope),
        "undefined"
    );
    execute(scope, application);
    let html = execute(scope, "renderBridge(AppBridge)");
    assert!(html.to_rust_string_lossy(scope).contains(">Ada</h1>"));
}

#[tokio::test]
async fn react_stream_pool_restores_request_contexts_for_repeated_calls() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/build-probe/tests/fixtures")
        .canonicalize()
        .unwrap();
    let output = build(&BuildConfig {
        root: root.clone(),
        server_entry: root.join("realm-application.tsx"),
        react_framework_entry: Some(root.join("realm-stream-framework.tsx")),
        client_entry: root.join("client.tsx"),
        css_entry: root.join("app.css"),
        asset_route: "/assets".into(),
    })
    .await
    .unwrap();
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
        1,
        Duration::from_secs(3),
    )
    .unwrap();
    for name in ["Ada", "Bea"] {
        let page = Page::from_json(format!(r#"{{"render":"ssr","title":"Test","language":"en","props":{{"name":"{name}"}},"state":{{"count":1}}}}"#).as_bytes()).unwrap();
        let (state, stream) = pool.render_stream(&page, "nonce_A").unwrap();
        assert_eq!(state.compact(), r#"{"count":1}"#);
        let bytes = stream.collect::<Result<Vec<_>, _>>().unwrap().concat();
        let html = std::str::from_utf8(&bytes).unwrap();
        assert!(html.contains(&format!(">{name}</h1>")), "{html}");
        assert!(!html.contains("setTimeout"));
    }
}

#[test]
fn application_snapshots_restore_in_separate_processes() {
    use std::process::{Command, Stdio};
    use wait_timeout::ChildExt;
    const FIXTURE: &str = "SSR_SNAPSHOT_PROCESS_CASE";
    const TEST: &str = "realm_tests::application_snapshots_restore_in_separate_processes";
    const FRAMEWORK: &str = "globalThis.__ssrReact = {}; globalThis.__ssrJsxRuntime = {}; globalThis.render = async (App, _props, state) => { const text = App(); let sent = false; return { stream: { getReader() { return { read() { if (sent) return Promise.resolve({done:true}); sent = true; return Promise.resolve({done:false, value:new TextEncoder().encode(text)}); } }; } }, state }; };";
    if let Ok(name) = std::env::var(FIXTURE) {
        let page = Page::from_json(
            br#"{"render":"ssr","title":"Test","language":"en","props":{},"state":null}"#,
        )
        .unwrap();
        if name == "plain" {
            let pool = Pool::new(ServerBundle {
                entry_path: "server/plain.js".into(),
                entry_bytes: b"export function render() { return {html:'<p>plain</p>', head:'', state:null}; }".to_vec(),
                chunks: Vec::new(),
            }, 4, 16, Duration::from_secs(3)).unwrap();
            for _ in 0..20 {
                assert_eq!(pool.render(&page).unwrap().html, b"<p>plain</p>");
            }
        } else {
            assert!(["Ada", "Bea", "Cy"].contains(&name.as_str()));
            let pool = Pool::new_react(
                ("server/framework.js".into(), FRAMEWORK.as_bytes().to_vec()),
                ServerBundle {
                    entry_path: "server/application.js".into(),
                    entry_bytes: format!("globalThis.__ssrApp = () => '<p>{name}</p>'")
                        .into_bytes(),
                    chunks: Vec::new(),
                },
                4,
                16,
                Duration::from_secs(3),
            )
            .unwrap();
            for _ in 0..20 {
                let (_, stream) = pool.render_stream(&page, "test_nonce").unwrap();
                assert_eq!(
                    stream.collect::<Result<Vec<_>, _>>().unwrap().concat(),
                    format!("<p>{name}</p>").as_bytes()
                );
            }
        }
        return;
    }
    let mut children: Vec<_> = ["Ada", "Bea", "Cy", "plain"]
        .into_iter()
        .map(|name| {
            Command::new(std::env::current_exe().unwrap())
                .args(["--exact", TEST, "--nocapture"])
                .env(FIXTURE, name)
                .stdin(Stdio::null())
                .spawn()
                .unwrap()
        })
        .collect();
    let mut failures = Vec::new();
    for (index, child) in children.iter_mut().enumerate() {
        match child.wait_timeout(Duration::from_secs(10)).unwrap() {
            Some(status) if status.success() => {}
            Some(status) => failures.push(format!("process {index} failed: {status}")),
            None => {
                child.kill().unwrap();
                child.wait().unwrap();
                failures.push(format!("process {index} timed out"));
            }
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}
