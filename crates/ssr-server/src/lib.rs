#![forbid(unsafe_code)]

use http::header::{ALLOW, CACHE_CONTROL, CONTENT_LENGTH, CONTENT_TYPE, ETAG, HeaderValue};
use http::{Method, Request, Response, StatusCode};
use ssr_build::PublicFiles;

pub fn serve_public(
    files: &PublicFiles,
    request: &Request<()>,
) -> Result<Response<Vec<u8>>, http::header::InvalidHeaderValue> {
    if request.method() != Method::GET && request.method() != Method::HEAD {
        let mut response = Response::new(Vec::new());
        *response.status_mut() = StatusCode::METHOD_NOT_ALLOWED;
        response
            .headers_mut()
            .insert(ALLOW, HeaderValue::from_static("GET, HEAD"));
        return Ok(response);
    }
    let Some(file) = files.get(request.uri().path()) else {
        let mut response = Response::new(Vec::new());
        *response.status_mut() = StatusCode::NOT_FOUND;
        return Ok(response);
    };
    let mut response = Response::new(if request.method() == Method::HEAD {
        Vec::new()
    } else {
        file.bytes().to_vec()
    });
    let headers = response.headers_mut();
    headers.insert(CONTENT_TYPE, HeaderValue::from_str(file.content_type())?);
    headers.insert(
        CONTENT_LENGTH,
        HeaderValue::from_str(&file.bytes().len().to_string())?,
    );
    headers.insert(
        ETAG,
        HeaderValue::from_str(&format!("\"{}\"", file.sha256()))?,
    );
    headers.insert(
        CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=31536000, immutable"),
    );
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    Ok(response)
}
