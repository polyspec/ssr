use super::source_event;
use notify::Event;
use notify::event::{AccessKind, CreateKind, EventKind, ModifyKind};
use std::path::PathBuf;

#[test]
fn mixed_paths_require_one_included_source_path() {
    let inputs = vec![
        (PathBuf::from("/app/web"), true),
        (PathBuf::from("/app/config.json"), false),
    ];
    let excluded = vec![PathBuf::from("/app/web/generated")];
    let event = Event::new(EventKind::Modify(ModifyKind::Any))
        .add_path("/app/web/generated/client.js".into())
        .add_path("/app/unrelated.txt".into());
    assert!(!super::input_event(&event, &excluded, &inputs));
    assert!(super::input_event(
        &event.add_path("/app/config.json".into()),
        &excluded,
        &inputs
    ));
}

#[test]
fn excluded_output_events_do_not_rebuild_but_new_sources_do() {
    let excluded = vec![
        PathBuf::from("/app/generated"),
        PathBuf::from("/app/node_modules"),
    ];
    let generated = Event::new(EventKind::Modify(ModifyKind::Any))
        .add_path("/app/generated/new/server.js".into());
    assert!(!source_event(&generated, &excluded));
    let source =
        Event::new(EventKind::Create(CreateKind::File)).add_path("/app/new-component.tsx".into());
    assert!(source_event(&source, &excluded));
    let mixed = generated.add_path("/app/App.tsx".into());
    assert!(source_event(&mixed, &excluded));
    assert!(source_event(&Event::new(EventKind::Any), &excluded));
    assert!(!source_event(
        &Event::new(EventKind::Access(AccessKind::Any)),
        &excluded
    ));
}
#[test]
fn full_change_queue_reports_the_rejected_result_and_stops_supervision() {
    use super::{DevelopmentError, ProcessOptions, Supervisor};
    use std::sync::{Arc, RwLock};
    use std::time::Duration;
    let current = Arc::new(RwLock::new(Err(DevelopmentError("preparing".into()))));
    let (changes, _receiver) = tokio::sync::mpsc::channel(1);
    let options = ProcessOptions {
        ready_timeout: Duration::from_secs(1),
        drain_timeout: Duration::from_secs(1),
        restart_limit: 0,
        event_capacity: 1,
        max_probe_bytes: 1,
    };
    let mut supervisor = Supervisor::new(
        Arc::clone(&current),
        changes,
        None,
        Box::new(|_, _| panic!("no process starts in this case")),
        Vec::new(),
        options,
    );
    supervisor.report(Ok(()), false);
    supervisor.report(
        Err(DevelopmentError("render preparation failed".into())),
        true,
    );
    assert!(supervisor.observer_failed);
    let error = current.read().unwrap().as_ref().err().unwrap().to_string();
    assert!(error.contains("change queue failed"), "{error}");
    assert!(error.contains("render preparation failed"), "{error}");
    assert_eq!(supervisor.errors.len(), 1);
}

#[test]
fn dropped_event_reports_rebuild_unless_only_excluded() {
    use notify::event::Flag;
    let excluded = vec![PathBuf::from("/app/node_modules")];
    let inputs = vec![(PathBuf::from("/app"), true)];
    let rescan = Event::new(EventKind::Other)
        .set_flag(Flag::Rescan)
        .add_path("/app".into());
    assert!(source_event(&rescan, &excluded));
    assert!(super::input_event(&rescan, &excluded, &inputs));
    let overflow = Event::new(EventKind::Other).set_flag(Flag::Rescan);
    assert!(source_event(&overflow, &excluded));
    assert!(super::input_event(&overflow, &excluded, &inputs));
    let excluded_rescan = Event::new(EventKind::Other)
        .set_flag(Flag::Rescan)
        .add_path("/app/node_modules/react".into());
    assert!(!source_event(&excluded_rescan, &excluded));
    assert!(!source_event(&Event::new(EventKind::Other), &excluded));
}

/// A build that the test holds: the build program waits on the hold FIFO, and opening the
/// FIFO for writing returns only when a build is waiting on it.
struct Hold(PathBuf);

impl Hold {
    async fn release(&self, then: impl FnOnce() + Send + 'static) {
        let fifo = self.0.clone();
        tokio::task::spawn_blocking(move || {
            let mut writer = std::fs::OpenOptions::new().write(true).open(&fifo).unwrap();
            then();
            std::io::Write::write_all(&mut writer, &[0]).unwrap();
        })
        .await
        .unwrap();
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn events_during_a_build_start_one_follow_up_build() {
    use super::{DevelopmentError, ProcessOptions, Supervisor};
    use std::sync::{Arc, RwLock};
    use std::time::Duration;
    let mut random = [0u8; 8];
    getrandom::fill(&mut random).unwrap();
    let name = random
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let directory = Directory(
        std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("ssr-coalesce-{name}")),
    );
    std::fs::create_dir(&directory.0).unwrap();
    let root = directory.0.clone();
    let source = root.join("source");
    std::fs::create_dir(&source).unwrap();
    std::fs::create_dir(root.join("output")).unwrap();
    std::fs::write(source.join("client.js"), "window.marker = 'one';").unwrap();
    std::fs::write(source.join("app.css"), "body { color: red; }").unwrap();
    let server = source.join("server.js");
    std::fs::write(
        &server,
        "export function render(_props, state) { return {html: '<p>one</p>', head: '', state}; }",
    )
    .unwrap();
    let hold = Hold(root.join("hold"));
    let status = std::process::Command::new("mkfifo")
        .arg(&hold.0)
        .status()
        .unwrap();
    assert!(status.success());
    let log = root.join("process.log");
    let program = crate::development::test_programs::program("SSR_DEVELOPMENT_PROCESS");
    let mut build = std::process::Command::new(&program);
    build
        .arg("build")
        .arg(&source)
        .arg(root.join("output"))
        .env("PROCESS_LOG", &log)
        .env("PROCESS_BUILD_HOLD", &hold.0);
    let render_log = log.clone();
    let current = Arc::new(RwLock::new(Err(DevelopmentError("preparing".into()))));
    let (changes, mut results) = tokio::sync::mpsc::channel(32);
    let supervisor = Supervisor::new(
        Arc::clone(&current),
        changes,
        Some((vec![source.clone()], Vec::new(), build)),
        Box::new(move |directory, socket| {
            let mut render = std::process::Command::new(&program);
            render
                .arg("render")
                .arg("-build")
                .arg(directory)
                .arg("-listen")
                .arg(socket)
                .env("PROCESS_LOG", &render_log);
            render
        }),
        br#"{"render":"ssr","title":"Page","language":"en","props":{},"state":null}"#.to_vec(),
        ProcessOptions {
            ready_timeout: Duration::from_secs(30),
            drain_timeout: Duration::from_secs(5),
            restart_limit: 1,
            event_capacity: 32,
            max_probe_bytes: 1024 * 1024,
        },
    );
    let (events, received) = tokio::sync::mpsc::channel(32);
    let (_failed, failure) = tokio::sync::watch::channel(None);
    let (_stop, cancel) = tokio::sync::watch::channel(false);
    let run = tokio::spawn(supervisor.run(Some((received, failure)), cancel));
    let modified = Event::new(EventKind::Modify(ModifyKind::Data(
        notify::event::DataChange::Content,
    )))
    .add_path(server.clone());
    let write = || Ok(modified.clone());
    events.send(write()).await.unwrap();
    // The first build waits on the hold FIFO; three events arrive while it runs.
    let during = events.clone();
    let event = modified.clone();
    hold.release(move || {
        for _ in 0..3 {
            during.try_send(Ok(event.clone())).unwrap();
        }
    })
    .await;
    results.recv().await.unwrap().unwrap();
    // The follow-up build waits on the hold FIFO; one event arrives while it runs.
    let during = events.clone();
    let event = write();
    hold.release(move || during.try_send(event).unwrap()).await;
    results.recv().await.unwrap().unwrap();
    // One more follow-up build for that event; later builds no longer wait.
    let path = hold.0.clone();
    hold.release(move || std::fs::remove_file(path).unwrap())
        .await;
    results.recv().await.unwrap().unwrap();
    // A write after the follow-up builds is still observed.
    events.send(write()).await.unwrap();
    results.recv().await.unwrap().unwrap();
    drop(events);
    run.await.unwrap().unwrap();
    let builds = std::fs::read_to_string(&log)
        .unwrap()
        .lines()
        .filter(|line| line.starts_with("build:"))
        .count();
    assert_eq!(
        builds, 4,
        "one build for the first event, one follow-up for each build with events, one for the last write"
    );
}

/// The directory of one case, removed when the case ends, also when an assertion fails.
struct Directory(PathBuf);

impl Drop for Directory {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
