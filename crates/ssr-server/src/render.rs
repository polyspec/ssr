use crate::{Body, BodyError, serve_public, stack};
use http::header::{ALLOW, CACHE_CONTROL, CONTENT_LENGTH, CONTENT_TYPE, HeaderValue};
use http::{Method, Request, Response, StatusCode};
use sha2::{Digest, Sha256};
use sourcemap::SourceMap;
use ssr_adapter_react::{Error as ReactError, ReactAdapter};
use ssr_adapter_vanilla::{Error as VanillaError, VanillaAdapter};
use ssr_build::{Build, BuildFile, PublicFiles, PublishError};
use ssr_core::{CALL_PATH, Page, Render};
use ssr_runtime::{Error as RuntimeError, Pool, ServerBundle};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::time::{Duration, Instant};

#[derive(Debug)]
pub enum Error {
    InvalidBuild(String),
    Public(PublishError),
    Runtime(RuntimeError),
    React(ReactError),
    Vanilla(VanillaError),
    SourceMap(sourcemap::Error),
    Header(http::header::InvalidHeaderValue),
    Body(BodyError),
    Random(getrandom::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidBuild(message) => write!(f, "invalid server build: {message}"),
            Self::Public(error) => write!(f, "public files failed: {error}"),
            Self::Runtime(error) => write!(f, "runtime failed: {error}"),
            Self::React(error) => write!(f, "React adapter failed: {error}"),
            Self::Vanilla(error) => write!(f, "vanilla adapter failed: {error}"),
            Self::SourceMap(error) => write!(f, "source map failed: {error}"),
            Self::Header(error) => write!(f, "HTTP header failed: {error}"),
            Self::Body(error) => write!(f, "HTML body failed: {error}"),
            Self::Random(error) => write!(f, "request nonce failed: {error}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidBuild(_) => None,
            Self::Public(error) => Some(error),
            Self::Runtime(error) => Some(error),
            Self::React(error) => Some(error),
            Self::Vanilla(error) => Some(error),
            Self::SourceMap(error) => Some(error),
            Self::Header(error) => Some(error),
            Self::Body(error) => Some(error),
            Self::Random(error) => Some(error),
        }
    }
}

#[derive(Clone, Copy)]
pub enum Adapter {
    React,
    Vanilla,
}

enum AdapterInstance {
    React(ReactAdapter),
    Vanilla(VanillaAdapter),
}

pub struct Server {
    public: PublicFiles,
    adapter: AdapterInstance,
    pool: Pool,
    source_maps: BTreeMap<String, SourceMap>,
}

fn verified<'a>(build: &'a Build, file: &BuildFile) -> Result<&'a [u8], Error> {
    let bytes = build
        .files
        .get(&file.path)
        .ok_or_else(|| Error::InvalidBuild(format!("missing file: {}", file.path)))?;
    let digest = Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    if file.sha256 != digest {
        return Err(Error::InvalidBuild(format!(
            "SHA-256 differs for {}",
            file.path
        )));
    }
    Ok(bytes)
}

fn response(
    status: StatusCode,
    content_type: &'static str,
    body: Vec<u8>,
) -> Result<Response<Body>, Error> {
    let length = body.len();
    let mut response = Response::new(Body::bytes(body));
    *response.status_mut() = status;
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static(content_type));
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    let length = HeaderValue::from_str(&length.to_string()).map_err(Error::Header)?;
    response.headers_mut().insert(CONTENT_LENGTH, length);
    response.headers_mut().insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    Ok(response)
}

fn stream_response(body: Body) -> Response<Body> {
    let mut response = Response::new(body);
    *response.status_mut() = StatusCode::OK;
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response.headers_mut().insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    response
}

fn request_nonce() -> Result<String, Error> {
    nonce_with(getrandom::fill).map_err(Error::Random)
}

fn nonce_with<E>(fill: impl FnOnce(&mut [u8]) -> Result<(), E>) -> Result<String, E> {
    let mut random = [0u8; 16];
    fill(&mut random)?;
    Ok(random.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn error_page(message: &str) -> Vec<u8> {
    format!("<!doctype html><html><head><meta charset=\"utf-8\"><title>Render failed</title></head><body><h1>Render failed</h1><pre>{}</pre></body></html>", escape_html(message)).into_bytes()
}

impl Server {
    pub fn new(
        build: &Build,
        adapter: Adapter,
        worker_count: usize,
        queue_capacity: usize,
        timeout: Duration,
    ) -> Result<Self, Error> {
        let public = PublicFiles::new(build).map_err(Error::Public)?;
        let server = &build.manifest.server;
        if server.url.is_some() || server.content_type != "text/javascript; charset=utf-8" {
            return Err(Error::InvalidBuild(format!(
                "server JavaScript must be private: {}",
                server.path
            )));
        }
        let entry_bytes = verified(build, server)?.to_vec();
        let mut chunks = Vec::with_capacity(build.manifest.server_chunks.len());
        for chunk in &build.manifest.server_chunks {
            if chunk.url.is_some() || chunk.content_type != "text/javascript; charset=utf-8" {
                return Err(Error::InvalidBuild(format!(
                    "server JavaScript must be private: {}",
                    chunk.path
                )));
            }
            chunks.push((chunk.path.clone(), verified(build, chunk)?.to_vec()));
        }
        let framework_file = match adapter {
            Adapter::React => {
                if !chunks.is_empty() {
                    return Err(Error::InvalidBuild(
                        "React server chunks are unsupported by the IIFE build".into(),
                    ));
                }
                Some(build.manifest.react_framework.as_ref().ok_or_else(|| {
                    Error::InvalidBuild("React framework bundle is missing".into())
                })?)
            }
            Adapter::Vanilla => {
                if build.manifest.react_framework.is_some() {
                    return Err(Error::InvalidBuild(
                        "vanilla build cannot include a React framework".into(),
                    ));
                }
                None
            }
        };
        let framework = if let Some(file) = framework_file {
            if file.url.is_some() || file.content_type != "text/javascript; charset=utf-8" {
                return Err(Error::InvalidBuild(
                    "React framework bundle must be private JavaScript".into(),
                ));
            }
            Some(verified(build, file)?.to_vec())
        } else {
            None
        };
        let expected_maps = std::iter::once(&build.manifest.server)
            .chain(framework_file)
            .chain(build.manifest.server_chunks.iter())
            .map(|file| format!("{}.map", file.path))
            .collect::<BTreeSet<_>>();
        let actual_maps = build
            .manifest
            .source_maps
            .iter()
            .map(|file| file.path.clone())
            .collect::<BTreeSet<_>>();
        if expected_maps != actual_maps || actual_maps.len() != build.manifest.source_maps.len() {
            return Err(Error::InvalidBuild(
                "server JavaScript and source maps must match exactly".into(),
            ));
        }
        let mut source_maps = BTreeMap::new();
        for map in &build.manifest.source_maps {
            if map.url.is_some() || map.content_type != "application/json; charset=utf-8" {
                return Err(Error::InvalidBuild(format!(
                    "source map must be private JSON: {}",
                    map.path
                )));
            }
            let bytes = verified(build, map)?;
            let decoded = SourceMap::from_slice(bytes).map_err(Error::SourceMap)?;
            let path = map.path.strip_suffix(".map").ok_or_else(|| {
                Error::InvalidBuild(format!("source map path is invalid: {}", map.path))
            })?;
            if source_maps.insert(path.to_owned(), decoded).is_some() {
                return Err(Error::InvalidBuild(format!(
                    "duplicate source map: {}",
                    map.path
                )));
            }
        }
        let client = build
            .manifest
            .client
            .url
            .as_deref()
            .ok_or_else(|| Error::InvalidBuild("client URL is missing".into()))?;
        let styles = build
            .manifest
            .styles
            .iter()
            .map(|style| {
                style.url.as_deref().ok_or_else(|| {
                    Error::InvalidBuild(format!("style URL is missing: {}", style.path))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let adapter = match adapter {
            Adapter::React => {
                AdapterInstance::React(ReactAdapter::new(client, &styles).map_err(Error::React)?)
            }
            Adapter::Vanilla => AdapterInstance::Vanilla(
                VanillaAdapter::new(client, &styles).map_err(Error::Vanilla)?,
            ),
        };
        let bundle = ServerBundle {
            entry_path: server.path.clone(),
            entry_bytes,
            chunks,
        };
        let pool = match (framework_file, framework) {
            (Some(file), Some(bytes)) => Pool::new_react(
                (file.path.clone(), bytes),
                bundle,
                worker_count,
                queue_capacity,
                timeout,
            ),
            (None, None) => Pool::new(bundle, worker_count, queue_capacity, timeout),
            _ => {
                return Err(Error::InvalidBuild(
                    "React framework verification is incomplete".into(),
                ));
            }
        }
        .map_err(Error::Runtime)?;
        Ok(Self {
            public,
            adapter,
            pool,
            source_maps,
        })
    }

    pub fn handle(&self, request: Request<Vec<u8>>) -> Result<Response<Body>, Error> {
        if request.uri().path() != CALL_PATH {
            let (parts, _) = request.into_parts();
            return serve_public(&self.public, &Request::from_parts(parts, ()))
                .map(|response| response.map(Body::bytes))
                .map_err(Error::Header);
        }
        if request.method() != Method::POST {
            let body = if request.method() == Method::HEAD {
                Vec::new()
            } else {
                b"POST required".to_vec()
            };
            let mut response = response(
                StatusCode::METHOD_NOT_ALLOWED,
                "text/plain; charset=utf-8",
                body,
            )?;
            response
                .headers_mut()
                .insert(ALLOW, HeaderValue::from_static("POST"));
            return Ok(response);
        }
        if request.uri().query().is_some() {
            return response(
                StatusCode::BAD_REQUEST,
                "text/plain; charset=utf-8",
                b"render query is not allowed".to_vec(),
            );
        }
        let content_type = request.headers().get(CONTENT_TYPE);
        if content_type != Some(&HeaderValue::from_static("application/json"))
            && content_type != Some(&HeaderValue::from_static("application/json; charset=utf-8"))
        {
            return response(
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "text/plain; charset=utf-8",
                b"application/json required".to_vec(),
            );
        }
        let page = match Page::from_json(request.body()) {
            Ok(page) => page,
            Err(error) => {
                return response(
                    StatusCode::BAD_REQUEST,
                    "text/plain; charset=utf-8",
                    error.to_string().into_bytes(),
                );
            }
        };
        let started = Instant::now();
        let nonce = match request_nonce() {
            Ok(nonce) => nonce,
            Err(error) => return self.render_error(error, started),
        };
        let rendered = match (&self.adapter, page.render) {
            (AdapterInstance::React(adapter), Render::Ssr) => {
                let (state, stream) = match self.pool.render_stream(&page, &nonce) {
                    Ok(result) => result,
                    Err(error) => return self.render_error(Error::Runtime(error), started),
                };
                let (prefix, suffix) = match adapter.stream_parts(&page, &state) {
                    Ok(parts) => parts,
                    Err(error) => return self.render_error(Error::React(error), started),
                };
                let metrics = stream.metrics();
                let body = match Body::stream(prefix, stream, suffix, &nonce) {
                    Ok(body) => body,
                    Err(error) => return self.render_error(Error::Body(error), started),
                };
                tracing::info!(
                    render_ms = started.elapsed().as_secs_f64() * 1000.0,
                    pool_wait_ms = metrics.pool_wait.as_secs_f64() * 1000.0,
                    heap_used_bytes = metrics.heap_used_bytes,
                    "React shell completed"
                );
                return Ok(stream_response(body));
            }
            (AdapterInstance::React(adapter), Render::Csr) => adapter
                .render(&page)
                .map(|result| (result, None))
                .map_err(Error::React),
            (AdapterInstance::Vanilla(adapter), _) => adapter
                .render_with_metrics(&page, &self.pool)
                .map_err(Error::Vanilla),
        };
        match rendered {
            Ok((result, metrics)) => {
                let bytes = match Body::html(result.html, &nonce).and_then(Body::collect_bytes) {
                    Ok(bytes) => bytes,
                    Err(error) => return self.render_error(Error::Body(error), started),
                };
                if let Some(metrics) = metrics {
                    tracing::info!(
                        render_ms = started.elapsed().as_secs_f64() * 1000.0,
                        pool_wait_ms = metrics.pool_wait.as_secs_f64() * 1000.0,
                        heap_used_bytes = metrics.heap_used_bytes,
                        "SSR document completed"
                    );
                } else {
                    tracing::info!(
                        render_ms = started.elapsed().as_secs_f64() * 1000.0,
                        "CSR document completed"
                    );
                }
                response(StatusCode::OK, "text/html; charset=utf-8", bytes)
            }
            Err(error) => self.render_error(error, started),
        }
    }

    fn render_error(&self, error: Error, started: Instant) -> Result<Response<Body>, Error> {
        let status = render_status(&error);
        let body = match &error {
            Error::Runtime(RuntimeError::JavaScript {
                message,
                stack: Some(raw_stack),
            })
            | Error::Vanilla(VanillaError::Runtime(RuntimeError::JavaScript {
                message,
                stack: Some(raw_stack),
            })) => match stack::map_stack(raw_stack, &self.source_maps) {
                Ok(mapped) => format!("JavaScript failed: {message}\n{mapped}"),
                Err(mapping_error) => format!(
                    "JavaScript failed: {message}\nstack mapping failed: {mapping_error}\n{raw_stack}"
                ),
            },
            _ => error.to_string(),
        };
        tracing::error!(render_ms = started.elapsed().as_secs_f64() * 1000.0, error = %body, "render failed");
        response(status, "text/html; charset=utf-8", error_page(&body))
    }
}

fn render_status(error: &Error) -> StatusCode {
    match error {
        Error::Runtime(
            RuntimeError::QueueFull
            | RuntimeError::WorkerStopped
            | RuntimeError::WorkerUnresponsive,
        ) => StatusCode::SERVICE_UNAVAILABLE,
        Error::Runtime(RuntimeError::Timeout) => StatusCode::GATEWAY_TIMEOUT,
        Error::Vanilla(VanillaError::Runtime(
            RuntimeError::QueueFull
            | RuntimeError::WorkerStopped
            | RuntimeError::WorkerUnresponsive,
        )) => StatusCode::SERVICE_UNAVAILABLE,
        Error::Vanilla(VanillaError::Runtime(RuntimeError::Timeout)) => StatusCode::GATEWAY_TIMEOUT,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

#[cfg(test)]
mod tests {
    use super::{Error, nonce_with, render_status};
    use http::StatusCode;
    use ssr_runtime::Error as RuntimeError;

    #[test]
    fn nonce_uses_all_random_bytes_and_reports_failure() {
        let nonce = nonce_with::<()>(|bytes| {
            for (index, byte) in bytes.iter_mut().enumerate() {
                *byte = index as u8;
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(nonce, "000102030405060708090a0b0c0d0e0f");
        assert_eq!(
            nonce_with::<&str>(|_| Err("random source failed")),
            Err("random source failed")
        );
    }

    #[test]
    fn render_failures_preserve_their_http_status() {
        assert_eq!(
            render_status(&Error::Runtime(RuntimeError::InvalidResult("shell failed"))),
            StatusCode::INTERNAL_SERVER_ERROR
        );
        assert_eq!(
            render_status(&Error::Runtime(RuntimeError::QueueFull)),
            StatusCode::SERVICE_UNAVAILABLE
        );
        assert_eq!(
            render_status(&Error::Runtime(RuntimeError::WorkerStopped)),
            StatusCode::SERVICE_UNAVAILABLE
        );
        assert_eq!(
            render_status(&Error::Runtime(RuntimeError::Timeout)),
            StatusCode::GATEWAY_TIMEOUT
        );
    }
}
