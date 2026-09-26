use std::collections::BTreeMap;

use http::header::{ALLOW, CONTENT_LENGTH, CONTENT_TYPE, ETAG};
use http::{Method, Request, StatusCode};
use sha2::{Digest, Sha256};
use ssr_build::{Build, BuildFile, Manifest, PublicFiles};
use ssr_server::serve_public;

fn files() -> PublicFiles {
    let bytes = b"window.app = true;".to_vec();
    let sha256 = Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let build = Build {
        manifest: Manifest {
            server: BuildFile {
                path: "server/app.js".into(),
                url: None,
                sha256: String::new(),
                content_type: "text/javascript; charset=utf-8".into(),
            },
            source_maps: Vec::new(),
            client: BuildFile {
                path: "client/app.js".into(),
                url: Some("/assets/app.js".into()),
                sha256,
                content_type: "text/javascript; charset=utf-8".into(),
            },
            styles: Vec::new(),
            server_chunks: Vec::new(),
            assets: Vec::new(),
        },
        files: BTreeMap::from([("client/app.js".into(), bytes)]),
    };
    PublicFiles::new(&build).unwrap()
}

#[test]
fn public_http_responses_have_exact_body_and_content_type() {
    let files = files();
    let request = Request::builder()
        .method(Method::GET)
        .uri("/assets/app.js")
        .body(())
        .unwrap();
    let response = serve_public(&files, &request).unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.body(), b"window.app = true;");
    assert_eq!(
        response.headers()[CONTENT_TYPE],
        "text/javascript; charset=utf-8"
    );
    assert_eq!(response.headers()[CONTENT_LENGTH], "18");
    assert!(response.headers().contains_key(ETAG));

    let request = Request::builder()
        .method(Method::HEAD)
        .uri("/assets/app.js")
        .body(())
        .unwrap();
    let response = serve_public(&files, &request).unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.body().is_empty());
    assert_eq!(response.headers()[CONTENT_LENGTH], "18");
}

#[test]
fn public_http_responses_reject_unknown_paths_and_methods() {
    let files = files();
    let request = Request::builder()
        .method(Method::GET)
        .uri("/server/app.js")
        .body(())
        .unwrap();
    assert_eq!(
        serve_public(&files, &request).unwrap().status(),
        StatusCode::NOT_FOUND
    );
    let request = Request::builder()
        .method(Method::POST)
        .uri("/assets/app.js")
        .body(())
        .unwrap();
    let response = serve_public(&files, &request).unwrap();
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(response.headers()[ALLOW], "GET, HEAD");
}
