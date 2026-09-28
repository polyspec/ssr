use super::{ContextModules, Sources, initialize, install_sources, render, resolve};
use crate::{Error, snapshot};
use deno_core::v8;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[test]
fn relative_paths_remain_within_server_files() {
    assert_eq!(
        resolve("server/sub/entry.js", "../chunk.js"),
        Some("server/chunk.js".into())
    );
    assert_eq!(resolve("server/entry.js", "../outside.js"), None);
    assert_eq!(resolve("server/entry.js", "chunk.js"), None);
    assert_eq!(resolve("server/entry.js", ".//chunk.js"), None);
    assert_eq!(resolve("server/entry.js", "./chunk:1.js"), None);
    assert_eq!(resolve("server/entry.js", "./chunk.js?query"), None);
}

#[test]
fn duplicate_and_invalid_files_fail() {
    assert!(
        Sources::new(
            (
                "server/entry.js".into(),
                b"export const render = 1".to_vec()
            ),
            vec![("server/entry.js".into(), b"other".to_vec())]
        )
        .is_err()
    );
    assert!(Sources::new(("other/entry.js".into(), b"x".to_vec()), vec![]).is_err());
    assert!(Sources::new(("server/chunk:1.js".into(), b"x".to_vec()), vec![]).is_err());
}

#[test]
fn snapshot_key_input_includes_entry_and_each_chunk() {
    let first = Sources::new(
        (
            "server/entry.js".into(),
            b"export function render() {}".to_vec(),
        ),
        vec![("server/chunk.js".into(), b"export const value = 1".to_vec())],
    )
    .unwrap();
    let changed_chunk = Sources::new(
        (
            "server/entry.js".into(),
            b"export function render() {}".to_vec(),
        ),
        vec![("server/chunk.js".into(), b"export const value = 2".to_vec())],
    )
    .unwrap();
    let changed_entry = Sources::new(
        ("server/chunk.js".into(), b"export const value = 1".to_vec()),
        vec![(
            "server/entry.js".into(),
            b"export function render() {}".to_vec(),
        )],
    )
    .unwrap();
    assert_ne!(first.cache_bytes(), changed_chunk.cache_bytes());
    assert_ne!(first.cache_bytes(), changed_entry.cache_bytes());
}

#[test]
fn static_import_cycle_evaluates_and_render_does_not_become_global() {
    deno_core::JsRuntime::init_platform(None);
    let mut creator = v8::Isolate::snapshot_creator(None, None);
    {
        v8::scope!(let scope, &mut creator);
        let context = v8::Context::new(scope, Default::default());
        scope.set_default_context(context);
        let scope = &mut v8::ContextScope::new(scope, context);
        let sources = Sources::new(
                (
                    "server/entry.js".into(),
                    b"import { value } from './a.js'; export function render() { return value(); }".to_vec(),
                ),
                vec![
                    (
                        "server/a.js".into(),
                        b"import { other } from './b.js'; export function value() { return other() + 1; }".to_vec(),
                    ),
                    (
                        "server/b.js".into(),
                        b"import { value } from './a.js'; export function other() { return 1; }".to_vec(),
                    ),
                ],
            )
            .unwrap();
        let indices = initialize(scope, context, Arc::new(sources)).unwrap();
        assert_eq!(indices.len(), 3);
        let global_key = v8::String::new(scope, "render").unwrap();
        assert!(
            context
                .global(scope)
                .get(scope, global_key.into())
                .unwrap()
                .is_undefined()
        );
        let function = render(scope, context).unwrap();
        let result = function
            .call(scope, context.global(scope).into(), &[])
            .unwrap();
        assert_eq!(result.int32_value(scope), Some(2));
        context.clear_all_slots();
    }
    assert!(
        creator
            .create_blob(v8::FunctionCodeHandling::Keep)
            .is_some()
    );
}

#[test]
fn dynamic_chunk_restores_in_three_workers_without_shared_state() {
    let sources = Sources::new(
            (
                "server/entry.js".into(),
                b"export async function render() { const first = await import('./chunk.js'); const second = await import('./chunk.js'); return `${first.value}:${second.value}:${globalThis.chunkCount}`; }".to_vec(),
            ),
            vec![(
                "server/chunk.js".into(),
                b"await Promise.resolve(); globalThis.chunkCount = (globalThis.chunkCount ?? 0) + 1; export const value = String(globalThis.chunkCount);".to_vec(),
            )],
        )
        .unwrap();
    let snapshot = snapshot::get(&sources, Duration::from_secs(5)).unwrap();
    let workers = (0..3)
        .map(|_| {
            let snapshot = std::sync::Arc::clone(&snapshot);
            thread::spawn(move || {
                let mut isolate = snapshot.isolate();
                v8::scope!(let scope, &mut isolate);
                (0..2)
                    .map(|_| {
                        let context = v8::Context::new(scope, Default::default());
                        let scope = &mut v8::ContextScope::new(scope, context);
                        let (sources, indices) = snapshot.module_data().unwrap();
                        install_sources(scope, context, sources, &indices).unwrap();
                        let _context_modules = ContextModules::new(context);
                        let function = render(scope, context).unwrap();
                        let value = function
                            .call(scope, context.global(scope).into(), &[])
                            .unwrap();
                        let promise = v8::Local::<v8::Promise>::try_from(value).unwrap();
                        scope.perform_microtask_checkpoint();
                        assert_eq!(promise.state(), v8::PromiseState::Fulfilled);
                        promise.result(scope).to_rust_string_lossy(scope)
                    })
                    .collect::<Vec<_>>()
            })
        })
        .collect::<Vec<_>>();
    for worker in workers {
        assert_eq!(
            worker.join().unwrap(),
            vec!["1:1:1".to_owned(), "1:1:1".to_owned()]
        );
    }
}

#[test]
fn incomplete_snapshot_module_index_is_an_error() {
    let sources = Sources::new(
        (
            "server/entry.js".into(),
            b"export function render() {}".to_vec(),
        ),
        vec![("server/chunk.js".into(), b"export const value = 1".to_vec())],
    )
    .unwrap();
    let snapshot = snapshot::get(&sources, Duration::from_secs(5)).unwrap();
    let (sources, mut indices) = snapshot.module_data().unwrap();
    indices.remove("server/chunk.js");
    let mut isolate = snapshot.isolate();
    v8::scope!(let scope, &mut isolate);
    let context = v8::Context::new(scope, Default::default());
    let scope = &mut v8::ContextScope::new(scope, context);
    assert!(matches!(
        install_sources(scope, context, sources, &indices),
        Err(Error::Snapshot("server module snapshot indices differ"))
    ));
}

#[test]
fn top_level_await_completes_or_fails_explicitly() {
    let complete = Sources::new(
        (
            "server/complete.js".into(),
            b"await Promise.resolve(); export function render() {}".to_vec(),
        ),
        vec![],
    )
    .unwrap();
    snapshot::get(&complete, Duration::from_secs(5)).unwrap();
}

#[test]
fn pending_top_level_await_is_rejected() {
    let pending = Sources::new(
        (
            "server/pending.js".into(),
            b"await new Promise(() => {}); export function render() {}".to_vec(),
        ),
        vec![],
    )
    .unwrap();
    assert!(matches!(
        snapshot::get(&pending, Duration::from_secs(5)),
        Err(Error::InvalidBundle(
            "server module evaluation did not complete"
        ))
    ));
}

#[test]
fn unexported_render_is_rejected() {
    let bundle = Sources::new(
        (
            "server/entry.js".into(),
            b"function render() { return {head:'', html:'wrong', state:null}; }".to_vec(),
        ),
        vec![],
    )
    .unwrap();
    assert!(matches!(
        snapshot::get(&bundle, Duration::from_secs(5)),
        Err(Error::InvalidBundle("render function export required"))
    ));
}

#[test]
fn missing_static_import_is_an_error() {
    let bundle = Sources::new(
        (
            "server/entry.js".into(),
            b"import './missing.js'; export function render() {}".to_vec(),
        ),
        vec![],
    )
    .unwrap();
    assert!(matches!(
        snapshot::get(&bundle, Duration::from_secs(5)),
        Err(Error::JavaScript { message, .. }) if message.contains("server module import is missing")
    ));
}

#[test]
fn import_attributes_are_rejected() {
    let bundle = Sources::new(
        (
            "server/entry.js".into(),
            b"import './chunk.js' with { type: 'json' }; export function render() {}".to_vec(),
        ),
        vec![(
            "server/chunk.js".into(),
            b"export const value = 1;".to_vec(),
        )],
    )
    .unwrap();
    assert!(matches!(
        snapshot::get(&bundle, Duration::from_secs(5)),
        Err(Error::JavaScript { message, .. }) if message.contains("module import attributes are unsupported")
    ));
}

#[test]
fn missing_dynamic_import_rejects_render() {
    let sources = Sources::new(
        (
            "server/entry.js".into(),
            b"export async function render() { await import('./missing.js'); }".to_vec(),
        ),
        vec![],
    )
    .unwrap();
    let snapshot = snapshot::get(&sources, Duration::from_secs(5)).unwrap();
    let mut isolate = snapshot.isolate();
    v8::scope!(let scope, &mut isolate);
    let context = v8::Context::new(scope, Default::default());
    let scope = &mut v8::ContextScope::new(scope, context);
    let (sources, indices) = snapshot.module_data().unwrap();
    install_sources(scope, context, sources, &indices).unwrap();
    let _context_modules = ContextModules::new(context);
    let function = render(scope, context).unwrap();
    let value = function
        .call(scope, context.global(scope).into(), &[])
        .unwrap();
    let promise = v8::Local::<v8::Promise>::try_from(value).unwrap();
    scope.perform_microtask_checkpoint();
    assert_eq!(promise.state(), v8::PromiseState::Rejected);
    assert!(
        promise
            .result(scope)
            .to_rust_string_lossy(scope)
            .contains("server module import is missing")
    );
    promise.mark_as_handled();
}

#[test]
fn unfinished_dynamic_chunk_rejects_render() {
    let sources = Sources::new(
        (
            "server/entry.js".into(),
            b"export async function render() { await import('./pending.js'); }".to_vec(),
        ),
        vec![(
            "server/pending.js".into(),
            b"await new Promise(() => {}); export const value = 1;".to_vec(),
        )],
    )
    .unwrap();
    let snapshot = snapshot::get(&sources, Duration::from_secs(5)).unwrap();
    let mut isolate = snapshot.isolate();
    v8::scope!(let scope, &mut isolate);
    let context = v8::Context::new(scope, Default::default());
    let scope = &mut v8::ContextScope::new(scope, context);
    let (sources, indices) = snapshot.module_data().unwrap();
    install_sources(scope, context, sources, &indices).unwrap();
    let _context_modules = ContextModules::new(context);
    let function = render(scope, context).unwrap();
    let value = function
        .call(scope, context.global(scope).into(), &[])
        .unwrap();
    let promise = v8::Local::<v8::Promise>::try_from(value).unwrap();
    scope.perform_microtask_checkpoint();
    assert_eq!(promise.state(), v8::PromiseState::Rejected);
    assert!(
        promise
            .result(scope)
            .to_rust_string_lossy(scope)
            .contains("server module evaluation did not complete")
    );
    promise.mark_as_handled();
}
