use http::{Request, Response};
use http_body_util::{BodyExt, Full, combinators::BoxBody};
use hyper::body::{Body, Bytes, Frame, Incoming, SizeHint};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use ssr_build::{Build, BuildConfig};
use ssr_server::{Adapter, Server};
use std::future::Future;
use std::io::Write;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::net::UnixListener;
use tokio::sync::watch;
use tokio::task::JoinSet;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

fn record(value: &str) -> Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(std::env::var("PROCESS_LOG")?)?;
    file.write_all(format!("{value}:{}\n", std::process::id()).as_bytes())?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    match args.first().map(String::as_str) {
        Some("build") if args.len() == 3 => {
            record("build")?;
            let root = PathBuf::from(&args[1]);
            let config = BuildConfig {
                root: root.clone(),
                server_entry: root.join("server.js"),
                react_framework_entry: None,
                client_entry: root.join("client.js"),
                css_entry: root.join("app.css"),
                asset_route: "/assets".into(),
                dependencies: None,
            };
            let output = ssr_build::build(&config)
                .await?
                .write(&PathBuf::from(&args[2]))?;
            println!("{}", output.display());
            Ok(())
        }
        Some("render") if args.len() == 5 && args[1] == "-build" && args[3] == "-listen" => {
            render(PathBuf::from(&args[2]), PathBuf::from(&args[4])).await
        }
        _ => Err("invalid process arguments".into()),
    }
}

async fn render(directory: PathBuf, socket: PathBuf) -> Result<()> {
    record("render")?;
    let build = Build::read(&directory)?;
    let server = Arc::new(Server::new(
        &build,
        Adapter::Vanilla,
        ssr_runtime::PoolOptions {
            worker_count: 2,
            queue_capacity: 16,
            timeout: Duration::from_secs(5),
            cleanup_timeout: Duration::from_secs(2),
            max_input_bytes: 16777216,
            max_queue_bytes: 67108864,
            max_chunk_bytes: 65536,
            max_output_bytes: 67108864,
            max_heap_bytes: 134217728,
        },
    )?);
    let listener = UnixListener::bind(&socket)?;
    println!("{}", socket.display());
    std::io::stdout().flush()?;
    let (stop, _) = watch::channel(false);
    let mut connections = JoinSet::new();
    let mut stdin = tokio::io::stdin();
    let mut input = Vec::new();
    loop {
        tokio::select! {
            input = stdin.read_to_end(&mut input) => {
                input?;
                if std::env::var_os("PROCESS_SHUTDOWN_STDOUT").is_some() { println!("extra"); std::io::stdout().flush()?; }
                if std::env::var_os("PROCESS_IGNORE_STOP").is_some() { std::future::pending::<()>().await; }
                break;
            },
            accepted = listener.accept() => {
                let (socket, _) = accepted?;
                let server = Arc::clone(&server);
                let mut stopped = stop.subscribe();
                connections.spawn(async move {
                    let service = service_fn(move |request| handle(Arc::clone(&server), request));
                    let connection = http1::Builder::new().serve_connection(TokioIo::new(socket), service);
                    tokio::pin!(connection);
                    tokio::select! {
                        result = &mut connection => result,
                        _ = stopped.changed() => {
                            connection.as_mut().graceful_shutdown();
                            connection.await
                        }
                    }
                });
            },
            result = connections.join_next(), if !connections.is_empty() => {
                if let Some(result) = result && let Err(error) = result? { eprintln!("connection failed: {error}"); }
            },
        }
    }
    drop(listener);
    stop.send_replace(true);
    while let Some(result) = connections.join_next().await {
        if let Err(error) = result? {
            eprintln!("connection failed during stop: {error}");
        }
    }
    record("stopped")?;
    Ok(())
}

async fn handle(
    server: Arc<Server>,
    request: Request<Incoming>,
) -> std::result::Result<Response<BoxBody<Bytes, std::io::Error>>, std::io::Error> {
    match request.uri().path() {
        "/host" => {
            let host = request
                .headers()
                .get(http::header::HOST)
                .ok_or_else(|| std::io::Error::other("Host missing"))?
                .as_bytes();
            return Ok(Response::new(
                Full::new(Bytes::copy_from_slice(host))
                    .map_err(|never| match never {})
                    .boxed(),
            ));
        }
        "/broken" => {
            return Ok(Response::new(
                BrokenBody {
                    first: true,
                    finish: Box::pin(tokio::time::sleep(Duration::from_millis(20))),
                }
                .boxed(),
            ));
        }
        "/crash" => std::process::exit(23),
        "/stream" => {
            return Ok(Response::new(
                OpenBody {
                    first: Some(Bytes::from_static(b"first")),
                }
                .boxed(),
            ));
        }
        _ => {}
    }
    if std::env::var_os("PROCESS_PREPARE_FAIL").is_some() {
        return Response::builder()
            .status(503)
            .body(
                Full::new(Bytes::from_static(b"not prepared"))
                    .map_err(|never| match never {})
                    .boxed(),
            )
            .map_err(std::io::Error::other);
    }
    let (parts, body) = request.into_parts();
    let body = body
        .collect()
        .await
        .map_err(std::io::Error::other)?
        .to_bytes()
        .to_vec();
    let response = tokio::task::spawn_blocking(move || -> Result<_> {
        let response = server.handle(Request::from_parts(parts, body))?;
        let (parts, body) = response.into_parts();
        Ok(Response::from_parts(
            parts,
            Full::new(Bytes::from(body.collect_bytes()?))
                .map_err(|never| match never {})
                .boxed(),
        ))
    })
    .await
    .map_err(std::io::Error::other)?
    .map_err(std::io::Error::other)?;
    Ok(response)
}

struct OpenBody {
    first: Option<Bytes>,
}
impl Body for OpenBody {
    type Data = Bytes;
    type Error = std::io::Error;
    fn poll_frame(
        mut self: Pin<&mut Self>,
        _: &mut Context<'_>,
    ) -> Poll<Option<std::result::Result<Frame<Bytes>, std::io::Error>>> {
        match self.first.take() {
            Some(bytes) => Poll::Ready(Some(Ok(Frame::data(bytes)))),
            None => Poll::Pending,
        }
    }
    fn size_hint(&self) -> SizeHint {
        SizeHint::default()
    }
}

impl Drop for OpenBody {
    fn drop(&mut self) {
        if let Err(error) = record("canceled") {
            eprintln!("cancellation record failed: {error}");
        }
    }
}

struct BrokenBody {
    first: bool,
    finish: Pin<Box<tokio::time::Sleep>>,
}
impl Body for BrokenBody {
    type Data = Bytes;
    type Error = std::io::Error;
    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<std::result::Result<Frame<Bytes>, Self::Error>>> {
        if self.first {
            self.first = false;
            return Poll::Ready(Some(Ok(Frame::data(Bytes::from_static(b"first")))));
        }
        match self.finish.as_mut().poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(()) => {
                Poll::Ready(Some(Err(std::io::Error::other("fixture response failed"))))
            }
        }
    }
    fn size_hint(&self) -> SizeHint {
        SizeHint::default()
    }
}
