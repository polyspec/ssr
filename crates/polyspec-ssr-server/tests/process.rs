use polyspec_ssr_core::Page;
use polyspec_ssr_server::{Development, ProcessOptions};
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

const PAGE: &[u8] = br#"{"render":"ssr","title":"Page","language":"en","props":{},"state":null}"#;

#[tokio::test]
async fn development_requires_an_absolute_source_root_before_running_commands() {
    let result = Development::start(
        vec![PathBuf::from("relative")],
        Vec::new(),
        Command::new("must-not-run"),
        |_, _| Command::new("must-not-run"),
        Page::from_json(PAGE).unwrap(),
        ProcessOptions {
            ready_timeout: Duration::from_secs(5),
            drain_timeout: Duration::from_secs(5),
            restart_limit: 0,
            event_capacity: 1,
            max_probe_bytes: 1,
        },
    )
    .await;
    assert!(result.err().unwrap().to_string().contains("absolute"));
}
