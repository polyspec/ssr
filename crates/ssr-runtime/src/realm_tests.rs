use crate::web;
use deno_core::v8;
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
    let mut bundles = Vec::new();
    for name in ["realm-framework.tsx", "realm-application.tsx"] {
        let output = build(&BuildConfig {
            root: root.clone(),
            server_entry: root.join(name),
            client_entry: root.join("client.tsx"),
            css_entry: root.join("app.css"),
            asset_route: "/assets".into(),
        })
        .await
        .unwrap();
        bundles.push(output.files[&output.manifest.server.path].clone());
    }
    let framework = std::str::from_utf8(&bundles[0]).unwrap();
    let application = std::str::from_utf8(&bundles[1]).unwrap();

    deno_core::JsRuntime::init_platform(None);
    let mut isolate = v8::Isolate::new(Default::default());
    v8::scope!(let scope, &mut isolate);
    let framework_context = v8::Context::new(scope, Default::default());
    let application_context = v8::Context::new(scope, Default::default());
    let app = {
        let scope = &mut v8::ContextScope::new(scope, application_context);
        web::install(scope, application_context).unwrap();
        execute(scope, application);
        let key = v8::String::new(scope, "AppBridge").unwrap();
        let value = application_context
            .global(scope)
            .get(scope, key.into())
            .unwrap();
        v8::Global::new(scope, value)
    };
    let scope = &mut v8::ContextScope::new(scope, framework_context);
    web::install(scope, framework_context).unwrap();
    execute(scope, "globalThis.setTimeout = function () {};");
    execute(scope, framework);
    let key = v8::String::new(scope, "AppBridge").unwrap();
    let app = v8::Local::new(scope, &app);
    assert_eq!(
        framework_context.global(scope).set(scope, key.into(), app),
        Some(true)
    );
    let html = execute(scope, "renderBridge(AppBridge)");
    assert_eq!(html.to_rust_string_lossy(scope), "<h1>Ada</h1>");
}
