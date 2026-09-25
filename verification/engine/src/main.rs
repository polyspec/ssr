fn main() {
    let _ = deno_core::JsRuntime::new(deno_core::RuntimeOptions {
        extensions: vec![
            deno_webidl::deno_webidl::init(),
            deno_web::deno_web::init(
                std::sync::Arc::new(deno_web::BlobStore::default()),
                None,
                false,
                Default::default(),
            ),
        ],
        ..Default::default()
    });
}
