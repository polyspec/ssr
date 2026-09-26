use crate::{serve_public, stack};
use http::header::{ALLOW, CACHE_CONTROL, CONTENT_LENGTH, CONTENT_TYPE, HeaderValue};
use http::{Method, Request, Response, StatusCode};
use sha2::{Digest, Sha256};
use sourcemap::SourceMap;
use ssr_adapter_react::{Error as ReactError, ReactAdapter};
use ssr_build::{Build, BuildFile, PublicFiles, PublishError};
use ssr_core::{CALL_PATH, Page};
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
    SourceMap(sourcemap::Error),
    Header(http::header::InvalidHeaderValue),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidBuild(message) => write!(f, "invalid server build: {message}"),
            Self::Public(error) => write!(f, "public files failed: {error}"),
            Self::Runtime(error) => write!(f, "runtime failed: {error}"),
            Self::React(error) => write!(f, "React adapter failed: {error}"),
            Self::SourceMap(error) => write!(f, "source map failed: {error}"),
            Self::Header(error) => write!(f, "HTTP header failed: {error}"),
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
            Self::SourceMap(error) => Some(error),
            Self::Header(error) => Some(error),
        }
    }
}

#[derive(Clone, Copy)]
pub enum Adapter {
    React,
}

enum AdapterInstance {
    React(ReactAdapter),
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
) -> Result<Response<Vec<u8>>, Error> {
    let mut response = Response::new(body);
    *response.status_mut() = status;
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static(content_type));
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    let length =
        HeaderValue::from_str(&response.body().len().to_string()).map_err(Error::Header)?;
    response.headers_mut().insert(CONTENT_LENGTH, length);
    response.headers_mut().insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    Ok(response)
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
        let expected_maps = std::iter::once(&build.manifest.server)
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
        };
        let bundle = ServerBundle {
            entry_path: server.path.clone(),
            entry_bytes,
            chunks,
        };
        let pool =
            Pool::new(bundle, worker_count, queue_capacity, timeout).map_err(Error::Runtime)?;
        Ok(Self {
            public,
            adapter,
            pool,
            source_maps,
        })
    }

    pub fn handle(&self, request: Request<Vec<u8>>) -> Result<Response<Vec<u8>>, Error> {
        if request.uri().path() != CALL_PATH {
            let (parts, _) = request.into_parts();
            return serve_public(&self.public, &Request::from_parts(parts, ()))
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
        let rendered = match &self.adapter {
            AdapterInstance::React(adapter) => adapter.render_with_metrics(&page, &self.pool),
        };
        let render_ms = started.elapsed().as_secs_f64() * 1000.0;
        match rendered {
            Ok((result, metrics)) => {
                if let Some(metrics) = metrics {
                    tracing::info!(
                        render_ms,
                        pool_wait_ms = metrics.pool_wait.as_secs_f64() * 1000.0,
                        heap_used_bytes = metrics.heap_used_bytes as u64,
                        "render completed"
                    );
                } else {
                    tracing::info!(render_ms, "CSR document completed");
                }
                response(StatusCode::OK, "text/html; charset=utf-8", result.html)
            }
            Err(error) => {
                let status = match &error {
                    ReactError::Runtime(
                        RuntimeError::QueueFull
                        | RuntimeError::WorkerStopped
                        | RuntimeError::WorkerUnresponsive,
                    ) => StatusCode::SERVICE_UNAVAILABLE,
                    ReactError::Runtime(RuntimeError::Timeout) => StatusCode::GATEWAY_TIMEOUT,
                    _ => StatusCode::INTERNAL_SERVER_ERROR,
                };
                let body = match &error {
                    ReactError::Runtime(RuntimeError::JavaScript {
                        message,
                        stack: Some(raw_stack),
                    }) => match stack::map_stack(raw_stack, &self.source_maps) {
                        Ok(mapped) => format!("JavaScript failed: {message}\n{mapped}"),
                        Err(mapping_error) => format!(
                            "JavaScript failed: {message}\nstack mapping failed: {mapping_error}\n{raw_stack}"
                        ),
                    },
                    _ => error.to_string(),
                };
                tracing::error!(render_ms, error = %body, "render failed");
                response(status, "text/plain; charset=utf-8", body.into_bytes())
            }
        }
    }
}
