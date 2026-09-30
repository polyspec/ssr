use http::{Request, Response};
use http_body_util::{BodyExt, Full};
use hyper::body::{Bytes, Incoming};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use std::convert::Infallible;
use std::io::Write;
use tokio::io::AsyncReadExt;
use tokio::net::UnixListener;
use tokio::task::JoinSet;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let path = std::env::args().nth(1).ok_or("socket path is missing")?;
    let listener = UnixListener::bind(&path)?;
    match std::env::var_os("SOCKET_DECLARE") {
        Some(declared) => println!("{}", std::path::PathBuf::from(declared).display()),
        None => println!("{path}"),
    }
    std::io::stdout().flush()?;
    let mut stdin = tokio::io::stdin();
    let mut input = Vec::new();
    let mut connections = JoinSet::new();
    loop {
        tokio::select! {
            input = stdin.read_to_end(&mut input) => { input?; break; },
            accepted = listener.accept() => {
                let (socket, _) = accepted?;
                connections.spawn(async move {
                    http1::Builder::new().serve_connection(TokioIo::new(socket), service_fn(handle)).await
                });
            },
            result = connections.join_next(), if !connections.is_empty() => {
                if let Some(result) = result && let Err(error) = result? {
                    eprintln!("socket connection failed: {error}");
                }
            },
        }
    }
    connections.abort_all();
    while let Some(result) = connections.join_next().await {
        match result {
            Ok(Err(error)) => eprintln!("socket connection failed during shutdown: {error}"),
            Ok(Ok(())) => {}
            Err(error) if error.is_cancelled() => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

async fn handle(request: Request<Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
    assert_eq!(request.method(), "POST");
    assert_eq!(request.uri().path(), "/_render");
    if request.uri().query().is_some() {
        assert_eq!(request.uri().query(), Some("case=socket"));
        assert_eq!(
            request.headers()[http::header::HOST],
            "app.example.test"
        );
    }
    assert_eq!(
        request.into_body().collect().await.unwrap().to_bytes(),
        "probe"
    );
    Ok(Response::builder()
        .header("content-type", "text/html")
        .body(Full::new(Bytes::from_static(b"<p>ready</p>")))
        .unwrap())
}
