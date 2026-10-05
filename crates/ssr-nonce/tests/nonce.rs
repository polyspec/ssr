use ssr_nonce::{Error, NonceRewriter};

fn rewritten(chunks: &[&[u8]], nonce: &str) -> Result<String, Error> {
    let mut rewriter = NonceRewriter::new(nonce);
    for chunk in chunks {
        rewriter.write(chunk)?;
    }
    rewriter.end()?;
    let mut result = Vec::new();
    while let Some(chunk) = rewriter.pop() {
        result.extend_from_slice(&chunk);
    }
    Ok(String::from_utf8(result).unwrap())
}

#[test]
fn public_rewriter_applies_the_request_nonce_to_inline_content() {
    let document = rewritten(
        &[
            b"<html><head><style>p{}</sty".as_slice(),
            b"le></head><body><script>run()</script></body></html>".as_slice(),
        ],
        "request_nonce",
    )
    .unwrap();
    assert!(
        document.contains("<style nonce=\"request_nonce\">p{}</style>"),
        "{document}"
    );
    assert!(
        document.contains("<script nonce=\"request_nonce\">run()</script>"),
        "{document}"
    );
    assert!(
        rewritten(
            &[b"<script nonce='other'>x()</script>".as_slice()],
            "request"
        )
        .is_err()
    );
}
