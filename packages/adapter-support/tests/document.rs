use polyspec_ssr_adapter_support::{Assets, HeadOutput, app_import, import_specifier};
use polyspec_ssr_core::{Page, Value};
use polyspec_ssr_runtime::{Pool, PoolOptions, ServerBundle};
use std::time::Duration;

const REJECTED: HeadOutput = HeadOutput::Rejected("head output is rejected");

fn page(render: &str, title: &str, props: &str) -> Page {
    Page::from_json(
        format!(
            r#"{{"render":"{render}","title":"{title}","language":"en","props":{props},"state":null}}"#
        )
        .as_bytes(),
    )
    .unwrap()
}

#[test]
fn document_escapes_the_page_and_orders_head_body_and_scripts() {
    let assets = Assets::new("/client.js", &["/a.css", "/b.css"], HeadOutput::Inserted).unwrap();
    let page = page("ssr", "A <b> & \\\"q\\\"", r#"{"x":"</script>"}"#);
    let document = assets
        .document(&page, b"<p>body</p>", b"<meta name=\"h\">", &page.state)
        .unwrap();
    let document = String::from_utf8(document).unwrap();
    let order = [
        "<title>A &lt;b&gt; &amp; &quot;q&quot;</title>",
        "<link rel=\"stylesheet\" href=\"/a.css\"><link rel=\"stylesheet\" href=\"/b.css\">",
        "<meta name=\"h\"></head>",
        "<div id=\"root\" data-render=\"ssr\"><p>body</p></div>",
        "{\"x\":\"\\u003c/script\\u003e\"}",
        "<script type=\"module\" src=\"/client.js\"></script></body></html>",
    ];
    let mut at = 0;
    for part in order {
        let found = document[at..].find(part);
        assert!(
            found.is_some(),
            "{part} is missing or out of order in {document}"
        );
        at += found.unwrap() + part.len();
    }
}

#[test]
fn a_rejecting_adapter_refuses_head_output_and_invalid_text() {
    let assets = Assets::new("/client.js", &[], REJECTED).unwrap();
    let page = page("ssr", "t", "{}");
    assert_eq!(
        assets.document(&page, b"", b"<meta>", &page.state).err(),
        Some("head output is rejected")
    );
    assert_eq!(
        assets.document(&page, &[0xff], b"", &page.state).err(),
        Some("rendered HTML must be UTF-8")
    );
    assert_eq!(
        assets.document(&page, b"", &[0xff], &page.state).err(),
        Some("rendered head must be UTF-8")
    );
    assert!(assets.document(&page, b"<p>", b"", &page.state).is_ok());
}

#[test]
fn assets_accept_only_local_javascript_and_css_urls() {
    for client in [
        "client.js",
        "//host/client.js",
        "/a/../client.js",
        "/client.css",
        "/c lient.js",
        "/a//b.js",
    ] {
        assert_eq!(
            Assets::new(client, &[], REJECTED).err(),
            Some("client URL must be a local JavaScript path"),
            "{client}"
        );
    }
    assert_eq!(
        Assets::new("/client.js", &["/style.js"], REJECTED).err(),
        Some("style URL must be a local CSS path")
    );
    assert!(Assets::new("/dir/client-1_2.js", &["/s.css"], REJECTED).is_ok());
}

#[test]
fn props_must_be_an_object() {
    let assets = Assets::new("/client.js", &[], REJECTED).unwrap();
    let mut page = page("csr", "t", "{}");
    page.props = Value::string("not an object");
    assert_eq!(
        assets.parts(&page, &page.state, "").err(),
        Some("props must be an object")
    );
}

#[test]
fn static_shell_requires_csr_empty_props_and_null_state() {
    let assets = Assets::new("/client.js", &[], REJECTED).unwrap();
    assert!(assets.static_shell(&page("csr", "t", "{}")).is_ok());
    for rejected in [page("ssr", "t", "{}"), page("csr", "t", r#"{"a":1}"#)] {
        assert_eq!(
            assets.static_shell(&rejected).err(),
            Some("static shell requires CSR, empty props and null state")
        );
    }
}

#[test]
fn application_paths_must_be_absolute() {
    assert_eq!(app_import("/app/x.js").unwrap(), "\"/app/x.js\"");
    assert_eq!(import_specifier("a\"b"), "\"a\\\"b\"");
    assert_eq!(
        app_import("app/x.js").err(),
        Some("application path must be absolute")
    );
}

#[derive(Debug)]
enum Failure {
    Input(&'static str),
    Runtime(polyspec_ssr_runtime::Error),
}

impl From<polyspec_ssr_runtime::Error> for Failure {
    fn from(error: polyspec_ssr_runtime::Error) -> Self {
        Self::Runtime(error)
    }
}

#[test]
fn render_uses_the_pool_for_ssr_and_an_empty_body_for_csr() {
    let bundle = ServerBundle {
        entry_path: "server/fixture.js".into(),
        entry_bytes: b"export function render(props, state) { return {head:'', html: '<main>ok</main>', state}; }".to_vec(),
        chunks: Vec::new(),
    };
    let pool = Pool::new(
        bundle,
        PoolOptions {
            worker_count: 1,
            queue_capacity: 0,
            timeout: Duration::from_secs(2),
            cleanup_timeout: Duration::from_secs(2),
            max_input_bytes: 16_777_216,
            max_queue_bytes: 67_108_864,
            max_chunk_bytes: 65_536,
            max_output_bytes: 67_108_864,
            max_heap_bytes: 134_217_728,
        },
    )
    .unwrap();
    let assets = Assets::new("/client.js", &[], HeadOutput::Rejected("rejected")).unwrap();
    let (ssr, metrics) = assets
        .render(&page("ssr", "t", "{}"), &pool, Failure::Input)
        .unwrap();
    assert!(
        String::from_utf8(ssr.html)
            .unwrap()
            .contains("<main>ok</main>")
    );
    assert!(metrics.is_some());
    let (csr, metrics) = assets
        .render(&page("csr", "t", "{}"), &pool, Failure::Input)
        .unwrap();
    assert!(
        String::from_utf8(csr.html)
            .unwrap()
            .contains("data-render=\"csr\"></div>")
    );
    assert!(metrics.is_none());
    let mut broken = page("csr", "t", "{}");
    broken.props = Value::string("not an object");
    match assets.render(&broken, &pool, Failure::Input) {
        Err(Failure::Input(message)) => assert_eq!(message, "props must be an object"),
        other => panic!("{other:?}"),
    }
    if let Err(Failure::Runtime(error)) =
        assets.render(&page("csr", "t", "{}"), &pool, Failure::Input)
    {
        panic!("{error}");
    }
}
