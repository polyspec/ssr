use ssr_core::{CALL_PATH, Error, Page, Render, RenderResult};

#[test]
fn page_fixtures_round_trip() {
    for source in [
        include_bytes!("fixtures/ssr.json").as_slice(),
        include_bytes!("fixtures/csr.json").as_slice(),
    ] {
        let page = Page::from_json(source).unwrap();
        let encoded = page.to_json().unwrap();
        let expected = ordered_json::parse_bytes(source).unwrap().compact();
        assert_eq!(encoded, expected.as_bytes());
        let decoded = Page::from_json(&encoded).unwrap();
        assert_eq!(decoded.render, page.render);
        assert_eq!(decoded.title, page.title);
        assert_eq!(decoded.language, page.language);
        assert_eq!(decoded.props.compact(), page.props.compact());
        assert_eq!(decoded.state.compact(), page.state.compact());
    }
}

#[test]
fn page_rejects_invalid_json_and_fields() {
    let invalid = [
        "{",
        "[]",
        "{}",
        r#"{"render":"html","title":"T","language":"en","props":{},"state":null}"#,
        r#"{"render":"ssr","title":"T","language":"en","props":[] ,"state":null}"#,
        r#"{"render":"ssr","title":1,"language":"en","props":{},"state":null}"#,
        r#"{"render":"ssr","title":"T","language":"en","props":{}}"#,
        r#"{"render":"ssr","title":"T","language":"en","props":{},"state":null,"extra":1}"#,
    ];
    for source in invalid {
        assert!(Page::from_json(source.as_bytes()).is_err(), "{source}");
    }
    assert!(matches!(
        Page::from_json(&[0xff]),
        Err(Error::InvalidJson(_))
    ));
    assert!(matches!(
        Page::from_json(br#"{"render":"csr","title":"T","language":"en","props":{}}"#),
        Err(Error::InvalidField("state"))
    ));
    assert_eq!(CALL_PATH, "/_render");
}

#[test]
fn page_preserves_state_and_render_result() {
    let source = include_bytes!("fixtures/ssr.json");
    let page = Page::from_json(source).unwrap();
    assert_eq!(page.render, Render::Ssr);
    assert_eq!(page.props.compact(), r#"{"b":1,"a":[true]}"#);
    assert_eq!(page.state.compact(), r#"{"selected":"선택"}"#);
    let output = ordered_json::parse(r#"{"selected":"output"}"#).unwrap();
    let result = RenderResult {
        html: b"<h1>News</h1>".to_vec(),
        state: output,
    };
    assert_eq!(result.html, b"<h1>News</h1>");
    assert_eq!(result.state.compact(), r#"{"selected":"output"}"#);
    assert_eq!(
        Error::Render("script failed".into()).to_string(),
        "render failed: script failed"
    );
}

#[test]
fn page_rejects_invalid_constructed_props() {
    let mut page = Page::from_json(include_bytes!("fixtures/csr.json")).unwrap();
    page.props = ordered_json::Value::null();
    assert!(matches!(page.to_json(), Err(Error::InvalidField("props"))));
}

#[test]
fn page_rejects_duplicate_object_keys_at_any_depth() {
    for source in [
        r#"{"render":"ssr","title":"T","language":"en","props":{},"state":null,"render":"csr"}"#,
        r#"{"render":"ssr","title":"T","language":"en","props":{"a":1,"a":2},"state":null}"#,
        r#"{"render":"ssr","title":"T","language":"en","props":{"a":1,"\u0061":2},"state":null}"#,
        r#"{"render":"csr","title":"T","language":"en","props":{},"state":[{"x":1,"x":2}]}"#,
    ] {
        assert!(Page::from_json(source.as_bytes()).is_err(), "{source}");
    }
}
