#[path = "../../polyspec-ssr-build/tests/fixture/mod.rs"]
mod fixture;
mod support;
use http::{Method, Request, StatusCode, header};
use polyspec_ssr_build::{BuildConfig, build};
use polyspec_ssr_server::{Adapter, Server};
use std::time::Duration;

#[tokio::test]
async fn dropping_http_body_cancels_render_and_releases_worker() {
    let fixture = fixture::Fixture::new();
    let root = fixture.root.clone();
    let generated = root.join("generated-ssr-server-cancellation");
    std::fs::create_dir_all(&generated).unwrap();
    let framework = generated.join("framework.tsx");
    let application = generated.join("application.tsx");
    let client = generated.join("client.tsx");
    std::fs::write(
        &framework,
        r#"globalThis.__ssrReact = {};
globalThis.__ssrJsxRuntime = {};
globalThis.render = async (_App, props, state) => ({
  state,
  stream: {
    getReader() {
      let reads = 0;
      return {
        read() {
          reads++;
          if (reads === 1) {
            return Promise.resolve({
              done: false,
              value: new TextEncoder().encode('<p>first</p>')
            });
          }
          if (props.loop) { for (;;) {} }
          return Promise.resolve({done:true});
        }
      };
    }
  }
});"#,
    )
    .unwrap();
    std::fs::write(&application, "globalThis.__ssrApp = () => '<p>first</p>';").unwrap();
    std::fs::write(&client, "window.lateReaderFixture = true;").unwrap();
    let output = build(&BuildConfig {
        root: root.clone(),
        server_entry: application,
        react_framework_entry: Some(framework),
        client_entry: client,
        css_entry: root.join("app.css"),
        asset_route: "/assets".into(),
        dependencies: Some(fixture.packages.clone()),
    })
    .await
    .unwrap();
    let server = Server::new(
        &output,
        Adapter::React,
        support::options(1, 1, Duration::from_secs(3)),
    )
    .unwrap();
    let request = Request::builder()
        .method(Method::POST)
        .uri("/_render")
        .header(header::CONTENT_TYPE, "application/json")
        .body(
            br#"{"render":"ssr","title":"Stream","language":"en","props":{"loop":true},"state":null}"#
                .to_vec(),
        )
        .unwrap();
    let response = server.handle(request).unwrap();
    let (parts, mut body) = response.into_parts();
    assert_eq!(parts.status, StatusCode::OK);
    assert_eq!(
        parts.headers[header::CONTENT_TYPE],
        "text/html; charset=utf-8"
    );
    assert!(!parts.headers.contains_key(header::CONTENT_LENGTH));

    let mut received = Vec::new();
    while !String::from_utf8_lossy(&received).contains("<p>first</p>") {
        received.extend_from_slice(&body.next().unwrap().unwrap());
    }
    drop(body);
    let request = Request::builder()
        .method(Method::POST)
        .uri("/_render")
        .header(header::CONTENT_TYPE, "application/json")
        .body(br#"{"render":"ssr","title":"Stream","language":"en","props":{"loop":false},"state":null}"#.to_vec())
        .unwrap();
    let response = server.handle(request).unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let document = response.into_body().collect_bytes().unwrap();
    assert!(
        String::from_utf8(document)
            .unwrap()
            .contains("<p>first</p>")
    );
}
