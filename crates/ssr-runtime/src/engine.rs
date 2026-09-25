use crate::{Command, Error, state_from_json, web};
use crossbeam_channel::Receiver;
use deno_core::{JsRuntime, RuntimeOptions, v8};
use ssr_core::RenderResult;
use std::sync::mpsc::SyncSender;

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
    source: &str,
    requests: Receiver<Command>,
    ready: SyncSender<Result<v8::IsolateHandle, Error>>,
) {
    let scheduler = match tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
    {
        Ok(scheduler) => scheduler,
        Err(error) => {
            let _ = ready.send(Err(Error::WorkerStartup(error)));
            return;
        }
    };
    let _entered = scheduler.enter();
    let mut runtime = JsRuntime::new(RuntimeOptions::default());
    let handle = runtime.v8_isolate().thread_safe_handle();
    let script = match compile(&mut runtime, source) {
        Ok(script) => script,
        Err(error) => {
            let _ = ready.send(Err(error));
            return;
        }
    };
    let serializer = match compile(
        &mut runtime,
        "(value) => JSON.stringify(value, function (_key, item) { if (item === undefined || typeof item === 'function' || typeof item === 'symbol' || typeof item === 'bigint' || (typeof item === 'number' && !Number.isFinite(item))) { throw new TypeError('render state contains a non-JSON value'); } return item; })",
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
                let result = render(&mut runtime, &script, &serializer, &props, &state);
                runtime.v8_isolate().cancel_terminate_execution();
                let _ = reply.send(result);
            }
            Command::Stop => break,
        }
    }
}

fn compile(runtime: &mut JsRuntime, source: &str) -> Result<v8::Global<v8::UnboundScript>, Error> {
    v8::scope!(let scope, runtime.v8_isolate());
    let context = v8::Context::new(scope, Default::default());
    let scope = &mut v8::ContextScope::new(scope, context);
    v8::tc_scope!(let scope, scope);
    let text = v8::String::new(scope, source)
        .ok_or(Error::InvalidBundle("bundle exceeds V8 string limit"))?;
    let mut source = v8::script_compiler::Source::new(text, None);
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
    runtime: &mut JsRuntime,
    compiled: &v8::Global<v8::UnboundScript>,
    serializer: &v8::Global<v8::UnboundScript>,
    props: &str,
    state: &str,
) -> Result<RenderResult, Error> {
    v8::scope!(let scope, runtime.v8_isolate());
    let context = v8::Context::new(scope, Default::default());
    let scope = &mut v8::ContextScope::new(scope, context);
    v8::tc_scope!(let scope, scope);
    web::install(scope, context)?;
    let script = v8::Local::new(scope, compiled).bind_to_current_context(scope);
    script.run(scope).ok_or_else(|| caught!(scope))?;
    let name =
        v8::String::new(scope, "render").ok_or(Error::InvalidBundle("render name unavailable"))?;
    let value = context
        .global(scope)
        .get(scope, name.into())
        .ok_or_else(|| caught!(scope))?;
    let function = v8::Local::<v8::Function>::try_from(value)
        .map_err(|_| Error::InvalidBundle("global render function required"))?;
    let props = parse(scope, props).ok_or_else(|| caught!(scope))?;
    let state = parse(scope, state).ok_or_else(|| caught!(scope))?;
    let value = function
        .call(scope, context.global(scope).into(), &[props, state])
        .ok_or_else(|| caught!(scope))?;
    if value.is_promise() {
        return Err(Error::InvalidResult("promise result is not synchronous"));
    }
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
    Ok(RenderResult { html, state })
}

fn parse<'s>(scope: &mut v8::PinScope<'s, '_>, input: &str) -> Option<v8::Local<'s, v8::Value>> {
    let text = v8::String::new(scope, input)?;
    v8::json::parse(scope, text)
}
