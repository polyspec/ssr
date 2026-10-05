#[path = "../../ssr-build/tests/fixture/mod.rs"]
mod fixture;
mod support;
use http::{Method, Request, StatusCode};
use ssr_adapter_react::{client_entry, framework_entry, server_entry};
use ssr_build::{BuildConfig, build};
use ssr_server::{Adapter, Server};
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Clone)]
struct TraceOutput(Arc<Mutex<Vec<u8>>>);

impl Write for TraceOutput {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[tokio::test]
async fn suspense_error_keeps_recovery_stream_and_records_request_error() {
    let fixture = fixture::Fixture::new();
    let root = fixture.root.clone();
    let generated = root.join("generated-ssr-server-react-stream");
    std::fs::create_dir_all(&generated).unwrap();
    let application = root.join("ReactStreamApp.tsx");
    let server_path = generated.join("server.tsx");
    let framework_path = generated.join("framework.tsx");
    let client_path = generated.join("client.tsx");
    std::fs::write(
        &server_path,
        server_entry(application.to_str().unwrap()).unwrap(),
    )
    .unwrap();
    std::fs::write(&framework_path, framework_entry()).unwrap();
    std::fs::write(
        &client_path,
        client_entry(application.to_str().unwrap()).unwrap(),
    )
    .unwrap();
    let mut output = build(&BuildConfig {
        root: root.clone(),
        server_entry: server_path,
        react_framework_entry: Some(framework_path),
        client_entry: client_path,
        css_entry: root.join("app.css"),
        asset_route: "/assets".into(),
        dependencies: Some(fixture.packages.clone()),
    })
    .await
    .unwrap();
    assert!(output.manifest.server_chunks.is_empty());
    let server = Server::new(
        &output,
        Adapter::React,
        support::options(1, 0, Duration::from_secs(10)),
    )
    .unwrap();
    let timeout_server = Server::new(
        &output,
        Adapter::React,
        support::options(1, 0, Duration::from_millis(200)),
    )
    .unwrap();
    let timeout_request = Request::builder()
        .method(Method::POST)
        .uri("/_render")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(br#"{"render":"ssr","title":"Stream","language":"en","props":{"failClient":false,"hangShell":true},"state":{"count":4}}"#.to_vec())
        .unwrap();
    let timeout_response = timeout_server.handle(timeout_request).unwrap();
    assert_eq!(timeout_response.status(), StatusCode::GATEWAY_TIMEOUT);
    let failed = Request::builder()
        .method(Method::POST)
        .uri("/_render")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(br#"{"render":"ssr","title":"Stream","language":"en","props":{"failClient":false,"failShell":true},"state":{"count":4}}"#.to_vec())
        .unwrap();
    let failed_response = server.handle(failed).unwrap();
    assert_eq!(failed_response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let failed_document =
        String::from_utf8(failed_response.into_body().collect_bytes().unwrap()).unwrap();
    assert!(
        failed_document.contains("before shell failure"),
        "{failed_document}"
    );
    output
        .manifest
        .server_chunks
        .push(output.manifest.server.clone());
    let error = match Server::new(
        &output,
        Adapter::React,
        support::options(1, 0, Duration::from_secs(10)),
    ) {
        Ok(_) => panic!("React server chunks must fail"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("React server chunks"), "{error}");
    let trace_bytes = Arc::new(Mutex::new(Vec::new()));
    tracing::subscriber::set_global_default(
        tracing_subscriber::fmt()
            .with_ansi(false)
            .with_writer({
                let trace_bytes = Arc::clone(&trace_bytes);
                move || TraceOutput(Arc::clone(&trace_bytes))
            })
            .finish(),
    )
    .unwrap();
    let request = Request::builder()
        .method(Method::POST)
        .uri("/_render")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(br#"{"render":"ssr","title":"Stream","language":"en","props":{"failClient":false},"state":{"count":4}}"#.to_vec())
        .unwrap();
    let response = server.handle(request).unwrap();
    if response.status() != StatusCode::OK {
        panic!(
            "unexpected status {}: {}",
            response.status(),
            String::from_utf8_lossy(&response.into_body().collect_bytes().unwrap())
        );
    }
    assert_eq!(
        response.headers()[http::header::CONTENT_TYPE],
        "text/html; charset=utf-8"
    );
    let mut body = response.into_body();
    let first = body.next().unwrap().unwrap();
    assert!(!first.is_empty());
    let mut chunks = vec![first];
    chunks.extend(body.collect::<Result<Vec<_>, _>>().unwrap());
    assert!(chunks.len() > 1, "React output must have multiple chunks");
    let document = String::from_utf8(chunks.concat()).unwrap();
    assert!(document.contains("id=\"fallback\""), "{document}");
    assert!(
        document.contains("<!--$!--><template data-msg="),
        "{document}"
    );
    assert!(document.contains("late server failure"), "{document}");
    let nonce = document
        .split("nonce=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    for tag in document.split("<script").skip(1) {
        let opening = tag.split('>').next().unwrap();
        assert!(opening.contains(&format!("nonce=\"{nonce}\"")), "{opening}");
    }
    let next_request = Request::builder()
        .method(Method::POST)
        .uri("/_render")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(br#"{"render":"ssr","title":"Stream","language":"en","props":{"failClient":false},"state":{"count":4}}"#.to_vec())
        .unwrap();
    let next_document = String::from_utf8(
        server
            .handle(next_request)
            .unwrap()
            .into_body()
            .collect_bytes()
            .unwrap(),
    )
    .unwrap();
    let next_nonce = next_document
        .split("nonce=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    assert_ne!(nonce, next_nonce);
    let held_request = Request::builder()
        .method(Method::POST)
        .uri("/_render")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(br#"{"render":"ssr","title":"Stream","language":"en","props":{"failClient":false},"state":{"count":4}}"#.to_vec())
        .unwrap();
    let held_response = server.handle(held_request).unwrap();
    assert_eq!(held_response.status(), StatusCode::OK);
    let overloaded_request = Request::builder()
        .method(Method::POST)
        .uri("/_render")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(br#"{"render":"ssr","title":"Stream","language":"en","props":{"failClient":false},"state":{"count":4}}"#.to_vec())
        .unwrap();
    let overloaded_response = server.handle(overloaded_request).unwrap();
    assert_eq!(
        overloaded_response.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    drop(held_response);
    let trace = String::from_utf8(trace_bytes.lock().unwrap().clone()).unwrap();
    assert!(trace.contains("pool_wait_ms="), "{trace}");
    assert!(trace.contains("heap_used_bytes="), "{trace}");
    assert!(trace.contains("React stream error"), "{trace}");
    assert!(trace.contains("late server failure"), "{trace}");
    assert!(trace.contains("component_stack="), "{trace}");
    assert!(trace.contains(nonce), "{trace}");
}
