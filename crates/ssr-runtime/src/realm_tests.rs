use crate::web;
use deno_core::v8;
use sha2::{Digest, Sha256};
use ssr_build::{BuildConfig, build};
use std::path::Path;

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
