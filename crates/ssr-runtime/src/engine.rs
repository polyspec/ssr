use crate::{
    Command, ContextRender, Error, WorkerReply, module, snapshot::Snapshot, state_from_json,
};
use crossbeam_channel::Receiver;
use deno_core::v8;
use ssr_core::RenderResult;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::mpsc::SyncSender;
use std::time::Instant;

macro_rules! caught {
    ($scope:expr) => {{
        if $scope.has_terminated() {
            Error::Timeout
        } else {
            let stack = $scope
                .stack_trace()
                .and_then(|v| v.to_string($scope))
                .map(|v| v.to_rust_string_lossy($scope));
            let message = $scope
                .exception()
                .and_then(|v| v.to_string($scope))
                .map(|v| v.to_rust_string_lossy($scope))
                .unwrap_or_else(|| "JavaScript execution failed without an exception".to_owned());
            Error::JavaScript { message, stack }
        }
    }};
}

pub(crate) fn worker(
    snapshot: Arc<Snapshot>,
    requests: Receiver<Command>,
    ready: SyncSender<Result<v8::IsolateHandle, Error>>,
) {
    let sources = snapshot.sources();
    let module_indices = snapshot.module_indices().clone();
    let mut runtime = snapshot.isolate();
    let handle = runtime.thread_safe_handle();
    let serializer = match compile(
        &mut runtime,
        "(value) => JSON.stringify(value, function (_key, item) { if (item === undefined || typeof item === 'function' || typeof item === 'symbol' || typeof item === 'bigint' || (typeof item === 'number' && !Number.isFinite(item))) { throw new TypeError('render state contains a non-JSON value'); } return item; })",
        "state.js",
    ) {
        Ok(script) => script,
        Err(error) => {
            let _ = ready.send(Err(error));
            return;
        }
    };
    if ready.send(Ok(handle)).is_err() {
        return;
    }
    while let Ok(command) = requests.recv() {
        match command {
            Command::Render {
                props,
                state,
                reply,
            } => {
                let result = render(
                    &mut runtime,
                    &serializer,
                    Arc::clone(&sources),
                    &module_indices,
                    &props,
                    &state,
                );
                let heap_used_bytes = runtime.get_heap_statistics().used_heap_size();
                runtime.cancel_terminate_execution();
                let _ = reply.send(WorkerReply {
                    result,
                    heap_used_bytes,
                });
            }
            Command::Stop => break,
        }
    }
}

fn compile(
    runtime: &mut v8::OwnedIsolate,
    source: &str,
    name: &str,
) -> Result<v8::Global<v8::UnboundScript>, Error> {
    v8::scope!(let scope, runtime);
    let context = v8::Context::new(scope, Default::default());
    let scope = &mut v8::ContextScope::new(scope, context);
    v8::tc_scope!(let scope, scope);
    let text = v8::String::new(scope, source)
        .ok_or(Error::InvalidBundle("bundle exceeds V8 string limit"))?;
    let name = v8::String::new(scope, name)
        .ok_or(Error::InvalidBundle("script name exceeds V8 string limit"))?;
    let origin = v8::ScriptOrigin::new(
        scope,
        name.into(),
        0,
        0,
        false,
        0,
        None,
        false,
        false,
        false,
        None,
    );
    let mut source = v8::script_compiler::Source::new(text, Some(&origin));
    let script = v8::script_compiler::compile_unbound_script(
        scope,
        &mut source,
        v8::script_compiler::CompileOptions::EagerCompile,
        v8::script_compiler::NoCacheReason::NoReason,
    )
    .ok_or(Error::InvalidBundle("bundle compilation failed"))?;
    Ok(v8::Global::new(scope, script))
}

fn render(
    runtime: &mut v8::OwnedIsolate,
    serializer: &v8::Global<v8::UnboundScript>,
    sources: Arc<module::Sources>,
    module_indices: &BTreeMap<String, usize>,
    props: &str,
    state: &str,
) -> Result<ContextRender, Error> {
    v8::scope!(let scope, runtime);
    let heap_before = scope.get_heap_statistics().used_heap_size() as i128;
    let reset_started = Instant::now();
    let context = v8::Context::new(scope, Default::default());
    let scope = &mut v8::ContextScope::new(scope, context);
    module::install_sources(scope, context, sources, module_indices)?;
    let _module_context = module::ContextModules::new(context);
    let context_reset = reset_started.elapsed();
    let context_heap_delta_bytes =
        scope.get_heap_statistics().used_heap_size() as i128 - heap_before;
    v8::tc_scope!(let scope, scope);
    let function = module::render(scope, context)?;
    let props = parse(scope, props).ok_or_else(|| caught!(scope))?;
    let state = parse(scope, state).ok_or_else(|| caught!(scope))?;
    let value = function
        .call(scope, context.global(scope).into(), &[props, state])
        .ok_or_else(|| caught!(scope))?;
    let value = settle(scope, value)?;
    let object = v8::Local::<v8::Object>::try_from(value)
        .map_err(|_| Error::InvalidResult("object required"))?;
    let html_name =
        v8::String::new(scope, "html").ok_or(Error::InvalidResult("html key unavailable"))?;
    let html = object
        .get(scope, html_name.into())
        .ok_or_else(|| caught!(scope))?;
    if !html.is_string() {
        return Err(Error::InvalidResult("html must be a string"));
    }
    let html_text = html.to_rust_string_lossy(scope);
    let html_roundtrip = v8::String::new(scope, &html_text)
        .ok_or(Error::InvalidResult("html exceeds V8 string limit"))?;
    if !html.strict_equals(html_roundtrip.into()) {
        return Err(Error::InvalidResult("html contains an unpaired surrogate"));
    }
    let html = html_text.into_bytes();
    let head_name =
        v8::String::new(scope, "head").ok_or(Error::InvalidResult("head key unavailable"))?;
    let head = object
        .get(scope, head_name.into())
        .ok_or_else(|| caught!(scope))?;
    if !head.is_string() {
        return Err(Error::InvalidResult("head must be a string"));
    }
    let head_text = head.to_rust_string_lossy(scope);
    let head_roundtrip = v8::String::new(scope, &head_text)
        .ok_or(Error::InvalidResult("head exceeds V8 string limit"))?;
    if !head.strict_equals(head_roundtrip.into()) {
        return Err(Error::InvalidResult("head contains an unpaired surrogate"));
    }
    let head = head_text.into_bytes();
    let state_name =
        v8::String::new(scope, "state").ok_or(Error::InvalidResult("state key unavailable"))?;
    let state = object
        .get(scope, state_name.into())
        .ok_or_else(|| caught!(scope))?;
    if state.is_undefined() {
        return Err(Error::InvalidResult("state is required"));
    }
    let serializer = v8::Local::new(scope, serializer).bind_to_current_context(scope);
    let serializer = serializer.run(scope).ok_or_else(|| caught!(scope))?;
    let serializer = v8::Local::<v8::Function>::try_from(serializer)
        .map_err(|_| Error::InvalidBundle("state serializer is not a function"))?;
    let json = serializer
        .call(scope, context.global(scope).into(), &[state])
        .ok_or_else(|| caught!(scope))?;
    let json = v8::Local::<v8::String>::try_from(json)
        .map_err(|_| Error::InvalidResult("state serialization did not return JSON"))?;
    let state = state_from_json(json.to_rust_string_lossy(scope).as_bytes())?;
    Ok(ContextRender {
        result: RenderResult { html, head, state },
        context_reset,
        context_heap_delta_bytes,
    })
}

fn settle<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    value: v8::Local<'s, v8::Value>,
) -> Result<v8::Local<'s, v8::Value>, Error> {
    let Ok(promise) = v8::Local::<v8::Promise>::try_from(value) else {
        return Ok(value);
    };
    scope.perform_microtask_checkpoint();
    if scope.is_execution_terminating() {
        return Err(Error::Timeout);
    }
    match promise.state() {
        v8::PromiseState::Fulfilled => Ok(promise.result(scope)),
        v8::PromiseState::Pending => {
            Err(Error::InvalidResult("promise has no scheduled completion"))
        }
        v8::PromiseState::Rejected => {
            let reason = promise.result(scope);
            promise.mark_as_handled();
            let message = reason
                .to_string(scope)
                .ok_or(Error::InvalidResult(
                    "promise rejection cannot be converted to text",
                ))?
                .to_rust_string_lossy(scope);
            let stack = if let Ok(object) = v8::Local::<v8::Object>::try_from(reason) {
                let key = v8::String::new(scope, "stack")
                    .ok_or(Error::InvalidResult("stack key unavailable"))?;
                let value = object.get(scope, key.into()).ok_or(Error::InvalidResult(
                    "promise rejection stack lookup failed",
                ))?;
                if value.is_undefined() {
                    None
                } else {
                    Some(
                        value
                            .to_string(scope)
                            .ok_or(Error::InvalidResult(
                                "promise rejection stack cannot be converted to text",
                            ))?
                            .to_rust_string_lossy(scope),
                    )
                }
            } else {
                None
            };
            Err(Error::JavaScript { message, stack })
        }
    }
}

fn parse<'s>(scope: &mut v8::PinScope<'s, '_>, input: &str) -> Option<v8::Local<'s, v8::Value>> {
    let text = v8::String::new(scope, input)?;
    v8::json::parse(scope, text)
}
