#[path = "support/development.rs"]
mod support;
use http::Request;
use http_body_util::BodyExt;
use ssr_core::Page;
use ssr_server::Development;
use std::fs;
use std::sync::atomic::Ordering;
use support::{Case, PAGE, change, contains, render};

#[tokio::test]
async fn changes_replace_render_and_public_files_and_fail_explicitly() {
    let case = Case::new();
    let (development, mut changes) = case.start().await.unwrap();
    assert!(render(&development).await.unwrap().contains("<p>one</p>"));
    assert_eq!(case.count("build"), 1);
    assert_eq!(case.count("render"), 1);
    let records = fs::read_to_string(&case.log).unwrap();
    let pids = records
        .lines()
        .map(|line| line.split(':').nth(1).unwrap())
        .collect::<Vec<_>>();
    assert_ne!(pids[0], pids[1]);
    assert!(
        pids.iter()
            .all(|pid| *pid != std::process::id().to_string())
    );
    case.write("two");
    contains(&development, &mut changes, "<p>two</p>").await;
    fs::write(case.source.join("server.js"), "export function render(;").unwrap();
    loop {
        if let Err(error) = change(&mut changes).await {
            assert!(error.to_string().contains("build process failed"));
            break;
        }
    }
    assert!(
        render(&development)
            .await
            .unwrap_err()
            .contains("build process failed")
    );
    case.write("three");
    contains(&development, &mut changes, "<p>three</p>").await;
    development.close().await.unwrap();
    assert_eq!(case.count("render"), case.count("stopped"));
    case.assert_sockets_removed(case.count("render"));
}

#[tokio::test]
async fn a_listening_process_is_not_ready_until_its_ssr_request_completes() {
    let case = Case::new();
    case.prepare_fail.store(true, Ordering::SeqCst);
    let error = case.start().await.err().unwrap();
    assert!(
        error
            .to_string()
            .contains("preparation requires HTTP 200 HTML"),
        "{error}"
    );
    assert!(
        error.to_string().contains("process exit collected"),
        "{error}"
    );
    case.assert_sockets_removed(1);
}

#[tokio::test]
async fn replacement_retains_the_previous_process_until_its_stream_is_dropped() {
    let case = Case::new();
    let (development, mut changes) = case.start().await.unwrap();
    let response = development
        .handle(Request::builder().uri("/stream").body(Vec::new()).unwrap())
        .await
        .unwrap();
    let mut body = Box::pin(response.into_body());
    let first = body.frame().await.unwrap().unwrap().into_data().unwrap();
    assert_eq!(first.as_ref(), b"first");
    case.write("two");
    contains(&development, &mut changes, "<p>two</p>").await;
    assert_eq!(case.count("stopped"), 0, "active response lost its process");
    drop(body);
    while case.count("stopped") == 0 {
        change(&mut changes).await.unwrap();
    }
    development.close().await.unwrap();
    assert_eq!(case.count("render"), case.count("stopped"));
    case.assert_sockets_removed(case.count("render"));
}

#[tokio::test]
async fn unexpected_process_exit_restarts_the_completed_build_without_rebuilding() {
    let case = Case::new();
    let (development, mut changes) = case.prepared().await.unwrap();
    case.write("source-changed-after-build");
    let response = development
        .handle(Request::builder().uri("/crash").body(Vec::new()).unwrap())
        .await;
    assert!(response.is_err());
    let error = change(&mut changes).await.unwrap_err();
    assert!(error.to_string().contains("exited unexpectedly"), "{error}");
    change(&mut changes).await.unwrap();
    assert!(render(&development).await.unwrap().contains("<p>one</p>"));
    assert_eq!(case.count("build"), 1);
    assert_eq!(case.count("render"), 2);
    development.close().await.unwrap();
    assert_eq!(case.count("stopped"), 1);
    case.assert_sockets_removed(2);
}

#[tokio::test]
async fn shutdown_reports_forced_termination_and_collects_process_exit() {
    let mut case = Case::new();
    case.ignore_stop = true;
    let (development, _changes) = case.start().await.unwrap();
    let error = development.close().await.unwrap_err();
    assert!(
        error
            .to_string()
            .contains("shutdown timed out; forced termination"),
        "{error}"
    );
    assert!(
        error.to_string().contains("process exit collected"),
        "{error}"
    );
    case.assert_sockets_removed(1);
}

#[tokio::test]
async fn dropping_the_supervisor_waits_for_its_child_process() {
    let case = Case::new();
    let (development, _changes) = case.prepared().await.unwrap();
    drop(development);
    assert_eq!(case.count("render"), case.count("stopped"));
    case.assert_sockets_removed(1);
}

#[tokio::test]
async fn shutdown_rejects_stdout_after_the_address_and_collects_exit() {
    let mut case = Case::new();
    case.shutdown_stdout = true;
    let (development, _changes) = case.prepared().await.unwrap();
    let error = development.close().await.unwrap_err();
    assert!(
        error
            .to_string()
            .contains("stdout contains output after its address"),
        "{error}"
    );
}

#[tokio::test]
async fn streaming_transport_errors_preserve_the_underlying_cause() {
    let case = Case::new();
    let (development, _changes) = case.prepared().await.unwrap();
    let response = development
        .handle(Request::builder().uri("/broken").body(Vec::new()).unwrap())
        .await
        .unwrap();
    let error = response.into_body().collect().await.unwrap_err();
    assert!(
        error
            .to_string()
            .contains("unexpected EOF during chunk size line"),
        "{error}"
    );
    development.close().await.unwrap();
}

#[tokio::test]
async fn automatic_replacement_stops_at_the_declared_restart_limit() {
    let case = Case::new();
    let (development, mut changes) = case.prepared().await.unwrap();
    for attempt in 0..2 {
        assert!(
            development
                .handle(Request::builder().uri("/crash").body(Vec::new()).unwrap())
                .await
                .is_err()
        );
        assert!(
            change(&mut changes)
                .await
                .unwrap_err()
                .to_string()
                .contains("exited unexpectedly")
        );
        let result = change(&mut changes).await;
        if attempt == 0 {
            result.unwrap();
        } else {
            assert!(
                result
                    .unwrap_err()
                    .to_string()
                    .contains("restart limit exhausted")
            );
        }
    }
    assert_eq!(case.count("render"), 2);
    assert!(
        render(&development)
            .await
            .unwrap_err()
            .contains("restart limit exhausted")
    );
    development.close().await.unwrap();
    case.assert_sockets_removed(2);
}

#[tokio::test]
async fn forwarding_preserves_the_application_host() {
    let case = Case::new();
    let (development, _changes) = case.prepared().await.unwrap();
    let response = development
        .handle(
            Request::builder()
                .uri("/host")
                .header(http::header::HOST, "service.example.test")
                .body(Vec::new())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(bytes.as_ref(), b"service.example.test");
    development.close().await.unwrap();
}

#[tokio::test]
async fn source_file_replacements_remain_watched() {
    let case = Case::new();
    let input = case.source.join("server.js");
    let replacement = case.source.join("replacement.js");
    let (development, mut changes) = Development::start(
        vec![input.clone()],
        Vec::new(),
        case.builder(),
        case.renderer(),
        Page::from_json(PAGE).unwrap(),
        case.options,
    )
    .await
    .unwrap();
    for value in ["two", "three"] {
        fs::write(&replacement, format!("export function render(_props, state) {{ return {{html: '<p>{value}</p>', head: '', state}}; }}")).unwrap();
        fs::rename(&replacement, &input).unwrap();
        contains(&development, &mut changes, &format!("<p>{value}</p>")).await;
    }
    development.close().await.unwrap();
}

#[tokio::test]
async fn preparation_enforces_the_declared_body_size() {
    let mut case = Case::new();
    case.options.max_probe_bytes = 1;
    let error = case.prepared().await.err().unwrap();
    assert!(
        error.to_string().contains("body exceeds 1 bytes"),
        "{error}"
    );
}

#[tokio::test]
async fn concurrent_close_calls_wait_for_the_same_process_result() {
    let case = Case::new();
    let (development, _changes) = case.prepared().await.unwrap();
    let (first, second) = tokio::join!(development.close(), development.close());
    first.unwrap();
    second.unwrap();
    development.close().await.unwrap();
    assert_eq!(case.count("render"), case.count("stopped"));
}

#[tokio::test]
async fn failed_rebuild_starts_the_previous_process_shutdown_duration() {
    let case = Case::new();
    let (development, mut changes) = case.start().await.unwrap();
    let response = development
        .handle(Request::builder().uri("/stream").body(Vec::new()).unwrap())
        .await
        .unwrap();
    let mut body = Box::pin(response.into_body());
    body.frame().await.unwrap().unwrap();
    fs::write(case.source.join("server.js"), "export function render(;").unwrap();
    let error = tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            if let Err(error) = change(&mut changes).await
                && error.to_string().contains("forced termination")
            {
                break error;
            }
        }
    })
    .await
    .expect("failed rebuild must start the configured shutdown duration");
    assert!(
        error.to_string().contains("process exit collected"),
        "{error}"
    );
    drop(body);
    assert!(
        development
            .close()
            .await
            .unwrap_err()
            .to_string()
            .contains("forced termination")
    );
}

#[tokio::test]
async fn dropping_a_response_cancels_the_child_response() {
    let case = Case::new();
    let (development, _changes) = case.prepared().await.unwrap();
    let response = development
        .handle(Request::builder().uri("/stream").body(Vec::new()).unwrap())
        .await
        .unwrap();
    let mut body = Box::pin(response.into_body());
    let first = body.frame().await.unwrap().unwrap().into_data().unwrap();
    assert_eq!(first.as_ref(), b"first");
    drop(body);
    development.close().await.unwrap();
    assert_eq!(case.count("canceled"), 1);
    assert_eq!(case.count("stopped"), 1);
    case.assert_sockets_removed(1);
}
