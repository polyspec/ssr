use super::*;
use http::header::CONTENT_TYPE;
use http_body_util::BodyExt;
use notify::event::{CreateKind, DataChange, EventKind, ModifyKind};
use std::time::Duration;

const PAGE: &[u8] = br#"{"render":"ssr","title":"Page","language":"en","props":{},"state":null}"#;

/// A file watch whose events arrive only when the test sends them. It reports each
/// registration to the test, so the test decides when an event arrives relative to `start`,
/// and it loses an event of a path that is not below a registered path.
struct Fake {
    calls: mpsc::UnboundedSender<(PathBuf, RecursiveMode)>,
    registered: Arc<Mutex<Vec<PathBuf>>>,
}

impl source::SourceWatch for Fake {
    fn watch(&mut self, path: &Path, mode: RecursiveMode) -> Result<(), DevelopmentError> {
        self.registered.lock().unwrap().push(path.to_path_buf());
        self.calls
            .send((path.to_path_buf(), mode))
            .map_err(|e| DevelopmentError(format!("fake watch registration failed: {e}")))
    }
}

struct Case {
    root: PathBuf,
    source: PathBuf,
    log: PathBuf,
}

impl Case {
    fn new() -> Self {
        let mut random = [0u8; 8];
        getrandom::fill(&mut random).unwrap();
        let name = random
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let root = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("ssr-start-{name}"));
        let source = root.join("source");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::create_dir(root.join("output")).unwrap();
        std::fs::write(source.join("client.js"), "window.marker = 'one';").unwrap();
        std::fs::write(source.join("app.css"), "body { color: red; }").unwrap();
        let case = Self {
            log: root.join("process.log"),
            root,
            source,
        };
        case.write("one");
        case
    }
    fn write(&self, value: &str) {
        std::fs::write(
            self.source.join("server.js"),
            format!("export function render(_props, state) {{ return {{html: '<p>{value}</p>', head: '', state}}; }}"),
        )
        .unwrap();
    }
    fn program() -> PathBuf {
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
        path
    }
    fn builds(&self) -> usize {
        match std::fs::read_to_string(&self.log) {
            Ok(records) => records
                .lines()
                .filter(|line| line.starts_with("build:"))
                .count(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
            Err(error) => panic!("process log failed: {error}"),
        }
    }
}

impl Drop for Case {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.root).unwrap();
    }
}

fn event(kind: EventKind, path: PathBuf) -> notify::Result<notify::Event> {
    Ok(notify::Event::new(kind).add_path(path))
}

async fn render(development: &Development) -> String {
    let request = Request::builder()
        .method("POST")
        .uri("/_render")
        .header(CONTENT_TYPE, "application/json")
        .body(PAGE.to_vec())
        .unwrap();
    let response = development.handle(request).await.unwrap();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    String::from_utf8(bytes.to_vec()).unwrap()
}

/// Events of writes made before the watch observes the source paths can arrive after the
/// watch registration returns. `start` must build only after its baseline event, must not
/// rebuild for events that arrived before it, and must observe a write made after it returns.
#[tokio::test]
async fn start_builds_once_from_the_watch_baseline() {
    let case = Case::new();
    let (calls_tx, mut calls) = mpsc::unbounded_channel();
    let handler = Arc::new(Mutex::new(None::<source::Handler>));
    let slot = Arc::clone(&handler);
    let registered = Arc::new(Mutex::new(Vec::new()));
    let watched = Arc::clone(&registered);
    let program = Case::program();
    let mut build = Command::new(&program);
    build
        .arg("build")
        .arg(&case.source)
        .arg(case.root.join("output"))
        .env("PROCESS_LOG", &case.log);
    let log = case.log.clone();
    let render_program = program.clone();
    let mut start = Box::pin(Development::start_with(
        vec![case.source.clone()],
        Vec::new(),
        build,
        move |directory: &Path, socket: &Path| {
            let mut render = Command::new(&render_program);
            render
                .arg("render")
                .arg("-build")
                .arg(directory)
                .arg("-listen")
                .arg(socket)
                .env("PROCESS_LOG", &log);
            render
        },
        Page::from_json(PAGE).unwrap(),
        ProcessOptions {
            ready_timeout: Duration::from_secs(30),
            drain_timeout: Duration::from_secs(5),
            restart_limit: 1,
            event_capacity: 32,
            max_probe_bytes: 1024 * 1024,
        },
        move |handler| {
            *slot.lock().unwrap() = Some(handler);
            Ok(Box::new(Fake {
                calls: calls_tx,
                registered: watched,
            }) as Box<dyn source::SourceWatch>)
        },
    ));
    let deliver = |event: notify::Result<notify::Event>| {
        let path = event.as_ref().unwrap().paths[0].clone();
        assert!(
            registered
                .lock()
                .unwrap()
                .iter()
                .any(|root| path.starts_with(root)),
            "the fake watch loses the event of {}, which is not registered",
            path.display()
        );
        (handler
            .lock()
            .unwrap()
            .as_ref()
            .expect("watch handler is missing"))(event)
    };
    let stale = case.source.join("server.js");
    let (root, mode) = tokio::select! {
        call = calls.recv() => call.expect("source watch registration is missing"),
        result = &mut start => panic!("start returned before any watch registration: {:?}", result.err()),
    };
    assert_eq!(
        (root, mode),
        (case.source.clone(), RecursiveMode::Recursive)
    );
    deliver(event(EventKind::Create(CreateKind::File), stale.clone()));
    let baseline = tokio::select! {
        call = calls.recv() => call.expect("baseline watch registration is missing"),
        result = &mut start => {
            let builds = case.builds();
            if let Ok((development, _)) = result {
                development.close().await.unwrap();
            }
            panic!("start returned without a watch baseline after {builds} builds; the event of a write made before the watch was forwarded")
        },
    };
    assert_eq!(baseline.1, RecursiveMode::NonRecursive);
    assert_eq!(case.builds(), 0, "start built before its watch baseline");
    deliver(event(
        EventKind::Modify(ModifyKind::Data(DataChange::Content)),
        stale.clone(),
    ));
    deliver(event(
        EventKind::Create(CreateKind::File),
        baseline.0.join(source::SENTINEL),
    ));
    let (development, mut changes) = start.await.unwrap();
    assert_eq!(case.builds(), 1);
    assert!(render(&development).await.contains("<p>one</p>"));
    case.write("two");
    deliver(event(
        EventKind::Modify(ModifyKind::Data(DataChange::Content)),
        stale,
    ));
    changes.recv().await.unwrap().unwrap();
    assert!(render(&development).await.contains("<p>two</p>"));
    assert_eq!(
        case.builds(),
        2,
        "events before the baseline started a build"
    );
    development.close().await.unwrap();
    assert!(!baseline.0.exists(), "the watch baseline directory remains");
}
