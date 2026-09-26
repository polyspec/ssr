use http::header::CONTENT_TYPE;
use http::{Method, Request, StatusCode};
use ssr_build::BuildConfig;
use ssr_server::{Adapter, Development};
use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const PAGE: &[u8] = br#"{"render":"ssr","title":"Page","language":"en","props":{},"state":null}"#;

fn render(development: &Development) -> Result<String, String> {
    let request = Request::builder()
        .method(Method::POST)
        .uri("/_render")
        .header(CONTENT_TYPE, "application/json")
        .body(PAGE.to_vec())
        .unwrap();
    let response = development
        .handle(request)
        .map_err(|error| error.to_string())?;
    assert_eq!(response.status(), StatusCode::OK);
    Ok(String::from_utf8(response.into_body()).unwrap())
}

fn client_url(document: &str) -> &str {
    let tail = document
        .rsplit("<script type=\"module\" src=\"")
        .next()
        .unwrap();
    tail.split('"').next().unwrap()
}

fn client(development: &Development, url: &str) -> Vec<u8> {
    development
        .handle(Request::builder().uri(url).body(Vec::new()).unwrap())
        .unwrap()
        .into_body()
}

fn write(path: &Path, source: &str) {
    fs::write(path, source).unwrap();
}

#[tokio::test]
async fn changes_replace_render_and_public_files_and_fail_explicitly() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/build-probe/tests/fixtures")
        .canonicalize()
        .unwrap();
    assert!(
        root.join("node_modules/react").is_dir(),
        "sample packages are required"
    );
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let server_entry = root.join(format!("development-{unique}.tsx"));
    let client_entry = root.join(format!("development-client-{unique}.tsx"));
    write(
        &server_entry,
        "globalThis.render = (_, state) => ({html:'<p>one</p>', state});",
    );
    write(&client_entry, "window.marker = 'one';");
    let config = BuildConfig {
        root: root.clone(),
        server_entry: server_entry.clone(),
        client_entry: client_entry.clone(),
        css_entry: root.join("app.css"),
        asset_route: "/assets".into(),
    };
    let (development, changes) =
        Development::start(config, Adapter::React, 1, 0, Duration::from_secs(10))
            .await
            .unwrap();
    let first = render(&development).unwrap();
    assert!(first.contains("<p>one</p>"), "{first}");
    let first_client = client(&development, client_url(&first));
    assert!(String::from_utf8_lossy(&first_client).contains("one"));

    write(
        &server_entry,
        "globalThis.render = (_, state) => ({html:'<p>two</p>', state});",
    );
    write(&client_entry, "window.marker = 'two';");
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        changes.recv_timeout(remaining).unwrap().unwrap();
        let next = render(&development).unwrap();
        if next.contains("<p>two</p>") && client_url(&first) != client_url(&next) {
            assert!(
                String::from_utf8_lossy(&client(&development, client_url(&next))).contains("two")
            );
            break;
        }
    }

    write(&server_entry, "globalThis.render = (;");
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    let failure = loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if let Err(error) = changes.recv_timeout(remaining).unwrap() {
            break error;
        }
    };
    assert!(failure.to_string().contains("JavaScript"), "{failure}");
    assert!(render(&development).unwrap_err().contains("JavaScript"));
    write(
        &server_entry,
        "globalThis.render = (_, state) => ({html:'<p>three</p>', state});",
    );
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if changes.recv_timeout(remaining).unwrap().is_ok()
            && render(&development).unwrap().contains("<p>three</p>")
        {
            break;
        }
    }
    drop(development);
    fs::remove_file(server_entry).unwrap();
    fs::remove_file(client_entry).unwrap();
}
