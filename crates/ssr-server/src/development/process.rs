use super::DevelopmentError;
use http::header::CONTENT_TYPE;
use http::{Request, Response, StatusCode, Uri};
use http_body_util::{BodyExt, Full};
use hyper::body::{Bytes, Incoming};
use hyper_util::client::legacy::{Client, connect::HttpConnector};
use hyper_util::rt::TokioExecutor;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout};
use tokio::sync::{mpsc, watch};

pub(super) enum Control {
    Stop,
    Retire,
}

pub(super) struct Generation {
    pub id: u64,
    uri: Uri,
    client: Client<HttpConnector, Full<Bytes>>,
    stop: mpsc::UnboundedSender<Control>,
}

impl Generation {
    pub async fn request(
        &self,
        mut request: Request<Vec<u8>>,
    ) -> Result<Response<Incoming>, DevelopmentError> {
        if request.uri().scheme().is_some() || request.uri().authority().is_some() {
            return Err(DevelopmentError(
                "request URI must contain only a path and query".into(),
            ));
        }
        let path = request
            .uri()
            .path_and_query()
            .ok_or_else(|| DevelopmentError("request path is missing".into()))?;
        let authority = self
            .uri
            .authority()
            .ok_or_else(|| DevelopmentError("render address has no authority".into()))?;
        *request.uri_mut() = format!("http://{authority}{path}")
            .parse()
            .map_err(|e| DevelopmentError(format!("request URI failed: {e}")))?;
        self.client
            .request(request.map(|body| Full::new(Bytes::from(body))))
            .await
            .map_err(|e| DevelopmentError(format!("render request failed: {e:?}")))
    }
}

impl Drop for Generation {
    fn drop(&mut self) {
        if self.stop.send(Control::Stop).is_err() {
            tracing::debug!(generation = self.id, "render process already exited");
        }
    }
}

pub(super) struct Process {
    pub generation: Arc<Generation>,
    pub stop: mpsc::UnboundedSender<Control>,
    child: Child,
    input: Option<ChildStdin>,
    output: BufReader<ChildStdout>,
    control: mpsc::UnboundedReceiver<Control>,
}

pub(super) struct Exit {
    pub id: u64,
    pub expected: bool,
    pub result: Result<(), DevelopmentError>,
}

impl Process {
    pub async fn start(
        id: u64,
        command: Command,
        probe: &[u8],
        timeout: Duration,
        max_probe_bytes: usize,
        mut cancel: watch::Receiver<bool>,
    ) -> Result<Self, DevelopmentError> {
        validate_program(&command)?;
        let mut child = tokio::process::Command::from(command)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| DevelopmentError(format!("render process start failed: {e}")))?;
        let input = child
            .stdin
            .take()
            .ok_or_else(|| DevelopmentError("render stdin is missing".into()))?;
        let pid = child.id();
        tracing::info!(generation = id, ?pid, "render process preparation started");
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| DevelopmentError("render stdout is missing".into()))?;
        let mut output = BufReader::new(stdout);
        let client = Client::builder(TokioExecutor::new()).build_http();
        let (stop, control) = mpsc::unbounded_channel();
        let result = tokio::select! {
            result = tokio::time::timeout(timeout, async {
                let uri: Uri = read_line(&mut output).await?.parse()
                    .map_err(|e| DevelopmentError(format!("render address is invalid: {e}")))?;
                if uri.scheme_str() != Some("http") || uri.authority().is_none() || uri.path() != "/" || uri.query().is_some() {
                    return Err(DevelopmentError("render address must be an absolute HTTP root URL".into()));
                }
                let generation = Arc::new(Generation { id, uri, client, stop: stop.clone() });
                let response = generation.request(Request::builder().method("POST").uri(ssr_core::CALL_PATH)
                    .header(CONTENT_TYPE, "application/json").body(probe.to_vec())
                    .map_err(|e| DevelopmentError(format!("preparation request failed: {e}")))?).await?;
                if response.status() != StatusCode::OK || response.headers().get(CONTENT_TYPE)
                    .and_then(|value| value.to_str().ok()).and_then(|value| value.split(';').next()) != Some("text/html") {
                    return Err(DevelopmentError(format!("render preparation requires HTTP 200 HTML, received {}", response.status())));
                }
                let mut body = response.into_body();
                let mut size = 0usize;
                while let Some(frame) = body.frame().await {
                    let frame = frame.map_err(|e| DevelopmentError(format!("render preparation body failed: {e:?}")))?;
                    if let Some(bytes) = frame.data_ref() {
                        size = size.checked_add(bytes.len()).ok_or_else(|| DevelopmentError("render preparation body size overflow".into()))?;
                        if size > max_probe_bytes { return Err(DevelopmentError(format!("render preparation body exceeds {max_probe_bytes} bytes"))); }
                    }
                }
                if size == 0 { return Err(DevelopmentError("render preparation body is empty".into())); }
                Ok(generation)
            }) => result.unwrap_or_else(|_| Err(DevelopmentError("render preparation timed out".into()))),
            _ = cancel.changed() => Err(DevelopmentError("render preparation canceled".into())),
        };
        match result {
            Ok(generation) => {
                tracing::info!(generation = id, ?pid, "render process preparation passed");
                Ok(Self {
                    generation,
                    stop,
                    child,
                    input: Some(input),
                    output,
                    control,
                })
            }
            Err(error) => Err(kill_and_wait(&mut child, error).await),
        }
    }

    pub async fn wait(mut self, timeout: Duration) -> Exit {
        let id = self.generation.id;
        drop(self.generation);
        let mut stdout_open = true;
        let mut expected = false;
        let mut status = None;
        let mut stop_at = None;
        let result = loop {
            if let Some(status) = &status
                && !stdout_open
            {
                break if expected && std::process::ExitStatus::success(status) {
                    Ok(())
                } else {
                    Err(DevelopmentError(format!(
                        "render process {id} {}: {status}",
                        if expected {
                            "stopped"
                        } else {
                            "exited unexpectedly"
                        }
                    )))
                };
            }
            tokio::select! {
                result = self.child.wait(), if status.is_none() => {
                    match result {
                        Ok(value) => { status = Some(value); stop_at.get_or_insert(tokio::time::Instant::now() + timeout); }
                        Err(error) => break Err(DevelopmentError(format!("render process {id} wait failed: {error}"))),
                    }
                },
                control = self.control.recv() => {
                    expected = true;
                    if matches!(control, Some(Control::Stop) | None) { self.input.take(); }
                    stop_at.get_or_insert(tokio::time::Instant::now() + timeout);
                },
                output = self.output.read_u8(), if stdout_open => match output {
                    Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => stdout_open = false,
                    Err(e) => break Err(kill_and_wait(&mut self.child, DevelopmentError(format!("render stdout failed: {e}"))).await),
                    Ok(byte) => break Err(kill_and_wait(&mut self.child, DevelopmentError(format!("render stdout contains output after its address: byte {byte:#04x}"))).await),
                },
                _ = async { match stop_at { Some(at) => tokio::time::sleep_until(at).await, None => std::future::pending().await } } => {
                    break Err(kill_and_wait(&mut self.child, DevelopmentError(format!("render process {id} shutdown timed out; forced termination"))).await);
                },
            }
        };
        match &result {
            Ok(()) => tracing::info!(generation = id, expected, "render process exit collected"),
            Err(error) => {
                tracing::error!(generation = id, expected, %error, "render process exit collected")
            }
        }
        Exit {
            id,
            expected,
            result,
        }
    }
}

pub(super) async fn build(
    command: &mut tokio::process::Command,
    mut cancel: watch::Receiver<bool>,
) -> Result<PathBuf, DevelopmentError> {
    let started = Instant::now();
    tracing::info!("development build started");
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| DevelopmentError(format!("build process start failed: {e}")))?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| DevelopmentError("build stdout is missing".into()))?
        .take(8193);
    let mut bytes = Vec::new();
    let result = tokio::select! {
        result = async {
            let (_, status) = tokio::try_join!(async {
                stdout.read_to_end(&mut bytes).await.map_err(|e| DevelopmentError(format!("build stdout failed: {e}")))?;
                if bytes.len() > 8192 { return Err(DevelopmentError("build path exceeds 8192 bytes".into())); }
                Ok(())
            }, async { child.wait().await.map_err(|e| DevelopmentError(format!("build wait failed: {e}"))) })?;
            if !status.success() { return Err(DevelopmentError(format!("build process failed: {status}"))); }
            if bytes.len() > 8192 { return Err(DevelopmentError("build path exceeds 8192 bytes".into())); }
            let line = std::str::from_utf8(&bytes).map_err(|e| DevelopmentError(format!("build path is not UTF-8: {e}")))?;
            let path = line.strip_suffix('\n').filter(|path| !path.is_empty() && !path.contains(['\n', '\r']))
                .ok_or_else(|| DevelopmentError("build stdout must contain one path and newline".into()))?;
            let path = PathBuf::from(path);
            if !path.is_absolute() || !path.canonicalize().is_ok_and(|canonical| canonical == path) {
                return Err(DevelopmentError("build directory must be absolute without symbolic links".into()));
            }
            Ok(path)
        } => result,
        _ = cancel.changed() => Err(DevelopmentError("development build canceled".into())),
    };
    let result = match result {
        Ok(path) => Ok(path),
        Err(error) => Err(kill_and_wait(&mut child, error).await),
    };
    match &result {
        Ok(path) => {
            tracing::info!(elapsed_ms = started.elapsed().as_millis() as u64, directory = %path.display(), "development build passed")
        }
        Err(error) => {
            tracing::error!(elapsed_ms = started.elapsed().as_millis() as u64, %error, "development build failed")
        }
    }
    result
}

async fn read_line(output: &mut BufReader<ChildStdout>) -> Result<String, DevelopmentError> {
    let mut line = Vec::new();
    loop {
        let byte = output
            .read_u8()
            .await
            .map_err(|e| DevelopmentError(format!("render address read failed: {e}")))?;
        if byte == b'\n' {
            break;
        }
        if line.len() == 8192 {
            return Err(DevelopmentError("render address exceeds 8192 bytes".into()));
        }
        line.push(byte);
    }
    String::from_utf8(line)
        .map_err(|e| DevelopmentError(format!("render address is not UTF-8: {e}")))
}

pub(super) fn validate_program(command: &Command) -> Result<(), DevelopmentError> {
    let program = Path::new(command.get_program());
    if !program.is_absolute()
        || !program.is_file()
        || !program.canonicalize().is_ok_and(|path| path == program)
    {
        return Err(DevelopmentError(
            "command program must be an absolute file without symbolic links".into(),
        ));
    }
    Ok(())
}

async fn kill_and_wait(child: &mut Child, error: DevelopmentError) -> DevelopmentError {
    let killed = child.start_kill();
    let waited = child.wait().await;
    match (killed, waited) {
        (Ok(()), Ok(status)) => {
            DevelopmentError(format!("{error}; process exit collected: {status}"))
        }
        (kill, wait) => DevelopmentError(format!(
            "{error}; process kill: {kill:?}; process wait: {wait:?}"
        )),
    }
}
