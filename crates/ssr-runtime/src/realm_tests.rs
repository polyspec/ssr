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
async fn react_renders_a_component_from_a_context_without_timers() {
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
    let framework_context = v8::Context::new(scope, Default::default());
    let application_context = v8::Context::new(scope, Default::default());
    let react = {
        let scope = &mut v8::ContextScope::new(scope, framework_context);
        web::install(scope, framework_context).unwrap();
        execute(scope, "globalThis.setTimeout = function () {};");
        execute(scope, framework);
        let key = v8::String::new(scope, "__ssrReact").unwrap();
        let value = framework_context
            .global(scope)
            .get(scope, key.into())
            .unwrap();
        v8::Global::new(scope, value)
    };
    let app = {
        let scope = &mut v8::ContextScope::new(scope, application_context);
        web::install(scope, application_context).unwrap();
        let key = v8::String::new(scope, "__ssrReact").unwrap();
        let react = v8::Local::new(scope, &react);
        assert_eq!(
            application_context
                .global(scope)
                .set(scope, key.into(), react),
            Some(true)
        );
        execute(scope, application);
        let key = v8::String::new(scope, "AppBridge").unwrap();
        let value = application_context
            .global(scope)
            .get(scope, key.into())
            .unwrap();
        v8::Global::new(scope, value)
    };
    let scope = &mut v8::ContextScope::new(scope, framework_context);
    let key = v8::String::new(scope, "AppBridge").unwrap();
    let app = v8::Local::new(scope, &app);
    assert_eq!(
        framework_context.global(scope).set(scope, key.into(), app),
        Some(true)
    );
    let html = execute(scope, "renderBridge(AppBridge)");
    assert!(html.to_rust_string_lossy(scope).contains(">Ada</h1>"));
}
