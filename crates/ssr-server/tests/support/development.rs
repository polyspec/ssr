use http::header::CONTENT_TYPE;
use http::{Request, StatusCode};
use http_body_util::BodyExt;
use ssr_core::Page;
use ssr_server::{Development, DevelopmentError, ProcessOptions};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;
use tokio::sync::mpsc::Receiver;

pub const PAGE: &[u8] =
    br#"{"render":"ssr","title":"Page","language":"en","props":{},"state":null}"#;
static NEXT: AtomicU64 = AtomicU64::new(1);

pub struct Case {
    pub root: PathBuf,
    pub source: PathBuf,
    pub output: PathBuf,
    pub log: PathBuf,
    pub prepare_fail: Arc<AtomicBool>,
    pub ignore_stop: bool,
    pub shutdown_stdout: bool,
    pub options: ProcessOptions,
}
impl Case {
    pub fn new() -> Self {
        static TRACING: std::sync::Once = std::sync::Once::new();
        TRACING.call_once(|| {
            tracing_subscriber::fmt()
                .with_writer(std::io::stderr)
                .try_init()
                .unwrap()
        });
        let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "ssr-development-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        fs::create_dir(&root).unwrap();
        let source = root.join("source");
        let output = root.join("output");
        fs::create_dir(&source).unwrap();
        fs::create_dir(&output).unwrap();
        fs::write(source.join("client.js"), "window.marker = 'one';").unwrap();
        fs::write(source.join("app.css"), "body { color: red; }").unwrap();
        let case = Self {
            log: root.join("process.log"),
            root,
            source,
            output,
            prepare_fail: Arc::new(AtomicBool::new(false)),
            ignore_stop: false,
            shutdown_stdout: false,
            options: ProcessOptions {
                ready_timeout: Duration::from_secs(5),
                drain_timeout: Duration::from_millis(300),
                restart_limit: 1,
                event_capacity: 32,
                max_probe_bytes: 1024 * 1024,
            },
        };
        case.write("one");
        case
    }
    pub fn write(&self, value: &str) {
        fs::write(self.source.join("server.js"), format!("export function render(_props, state) {{ return {{html: '<p>{value}</p>', head: '', state}}; }}")).unwrap();
    }
    pub fn program() -> PathBuf {
        let path = std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("examples/development_process");
        assert!(
            path.is_file(),
            "build the maintained development_process example before this test: {}",
            path.display()
        );
        path.canonicalize().unwrap()
    }
    pub fn builder(&self) -> Command {
        let mut build = Command::new(Self::program());
        build
            .arg("build")
            .arg(&self.source)
            .arg(&self.output)
            .env("PROCESS_LOG", &self.log);
        build
    }
    pub fn renderer(&self) -> impl Fn(&Path) -> Command + Send + Sync + 'static {
        let log = self.log.clone();
        let fail = Arc::clone(&self.prepare_fail);
        let ignore_stop = self.ignore_stop;
        let shutdown_stdout = self.shutdown_stdout;
        move |directory: &Path| {
            let mut render = Command::new(Self::program());
            render
                .arg("render")
                .arg("-build")
                .arg(directory)
                .env("PROCESS_LOG", &log);
            if fail.load(Ordering::SeqCst) {
                render.env("PROCESS_PREPARE_FAIL", "1");
            }
            if ignore_stop {
                render.env("PROCESS_IGNORE_STOP", "1");
            }
            if shutdown_stdout {
                render.env("PROCESS_SHUTDOWN_STDOUT", "1");
            }
            render
        }
    }
    pub async fn start(
        &self,
    ) -> Result<(Development, Receiver<Result<(), DevelopmentError>>), DevelopmentError> {
        Development::start(
            vec![self.source.clone()],
            Vec::new(),
            self.builder(),
            self.renderer(),
            Page::from_json(PAGE).unwrap(),
            self.options,
        )
        .await
    }
    pub async fn prepared(
        &self,
    ) -> Result<(Development, Receiver<Result<(), DevelopmentError>>), DevelopmentError> {
        let output = tokio::process::Command::from(self.builder())
            .output()
            .await
            .unwrap();
        eprint!("{}", String::from_utf8(output.stderr).unwrap());
        assert!(output.status.success(), "{}", output.status);
        let directory = PathBuf::from(
            String::from_utf8(output.stdout)
                .unwrap()
                .strip_suffix('\n')
                .unwrap(),
        );
        Development::from_build(
            directory,
            self.renderer(),
            Page::from_json(PAGE).unwrap(),
            self.options,
        )
        .await
    }
    pub fn count(&self, stage: &str) -> usize {
        fs::read_to_string(&self.log)
            .unwrap()
            .lines()
            .filter(|line| line.starts_with(&format!("{stage}:")))
            .count()
    }
}
impl Drop for Case {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

pub async fn render(development: &Development) -> Result<String, String> {
    let request = Request::builder()
        .method("POST")
        .uri("/_render")
        .header(CONTENT_TYPE, "application/json")
        .body(PAGE.to_vec())
        .unwrap();
    let response = development
        .handle(request)
        .await
        .map_err(|e| e.to_string())?;
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response
        .into_body()
        .collect()
        .await
        .map_err(|e| e.to_string())?
        .to_bytes();
    String::from_utf8(bytes.to_vec()).map_err(|e| e.to_string())
}
pub async fn change(
    changes: &mut Receiver<Result<(), DevelopmentError>>,
) -> Result<(), DevelopmentError> {
    tokio::time::timeout(Duration::from_secs(15), changes.recv())
        .await
        .expect("change event timed out")
        .expect("change receiver stopped")
}
pub async fn contains(
    development: &Development,
    changes: &mut Receiver<Result<(), DevelopmentError>>,
    marker: &str,
) {
    loop {
        change(changes).await.unwrap();
        if render(development)
            .await
            .is_ok_and(|value| value.contains(marker))
        {
            break;
        }
    }
}
