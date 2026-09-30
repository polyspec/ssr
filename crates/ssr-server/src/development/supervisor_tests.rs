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
