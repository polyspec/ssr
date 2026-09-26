use crate::{Error, StreamEvent, engine, state_from_json, web::FrameworkScheduler};
use crossbeam_channel::Sender;
use deno_core::v8;

macro_rules! caught {
    ($scope:expr) => {{
        if $scope.is_execution_terminating() {
            Error::Timeout
        } else {
            let stack = $scope
                .stack_trace()
                .and_then(|value| value.to_string($scope))
                .map(|value| value.to_rust_string_lossy($scope));
            let message = $scope
                .exception()
                .and_then(|value| value.to_string($scope))
                .map(|value| value.to_rust_string_lossy($scope))
                .unwrap_or_else(|| "JavaScript execution failed without an exception".to_owned());
            Error::JavaScript { message, stack }
        }
    }};
}

fn property<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    object: v8::Local<'s, v8::Object>,
    name: &str,
) -> Result<v8::Local<'s, v8::Value>, Error> {
    v8::tc_scope!(let scope, scope);
    let key = v8::String::new(scope, name)
        .ok_or(Error::InvalidResult("stream property name unavailable"))?;
    object.get(scope, key.into()).ok_or_else(|| caught!(scope))
}

fn call<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    receiver: v8::Local<'s, v8::Object>,
    name: &str,
    args: &[v8::Local<'s, v8::Value>],
) -> Result<v8::Local<'s, v8::Value>, Error> {
    v8::tc_scope!(let scope, scope);
    let value = property(scope, receiver, name)?;
    let function = v8::Local::<v8::Function>::try_from(value)
        .map_err(|_| Error::InvalidResult("stream function required"))?;
    function
        .call(scope, receiver.into(), args)
        .ok_or_else(|| caught!(scope))
}

fn finish_promise<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    context: v8::Local<'s, v8::Context>,
    scheduler: &FrameworkScheduler,
    value: v8::Local<'s, v8::Value>,
) -> Result<v8::Local<'s, v8::Value>, Error> {
    v8::tc_scope!(let scope, scope);
    let promise = v8::Local::<v8::Promise>::try_from(value)
        .map_err(|_| Error::InvalidResult("React stream Promise required"))?;
    loop {
        scope.perform_microtask_checkpoint();
        if scope.is_execution_terminating() {
            return Err(Error::Timeout);
        }
        if promise.state() != v8::PromiseState::Pending {
            break;
        }
        let Some(callback) = scheduler.next() else {
            return Err(Error::InvalidResult(
                "React stream has no scheduled completion",
            ));
        };
        let callback = v8::Local::new(scope, &callback);
        callback
            .call(scope, context.global(scope).into(), &[])
            .ok_or_else(|| caught!(scope))?;
    }
    engine::settle(scope, value)
}

fn state(
    scope: &mut v8::PinScope<'_, '_>,
    context: v8::Local<v8::Context>,
    serializer: &v8::Global<v8::UnboundScript>,
    value: v8::Local<v8::Value>,
) -> Result<ssr_core::Value, Error> {
    v8::tc_scope!(let scope, scope);
    if value.is_undefined() {
        return Err(Error::InvalidResult("state is required"));
    }
    let serializer = v8::Local::new(scope, serializer)
        .bind_to_current_context(scope)
        .run(scope)
        .ok_or_else(|| caught!(scope))?;
    let serializer = v8::Local::<v8::Function>::try_from(serializer)
        .map_err(|_| Error::InvalidBundle("state serializer is not a function"))?;
    let json = serializer
        .call(scope, context.global(scope).into(), &[value])
        .ok_or_else(|| caught!(scope))?;
    let json = v8::Local::<v8::String>::try_from(json)
        .map_err(|_| Error::InvalidResult("state serialization did not return JSON"))?;
    state_from_json(json.to_rust_string_lossy(scope).as_bytes())
}

pub(crate) fn render(
    runtime: &mut v8::OwnedIsolate,
    serializer: &v8::Global<v8::UnboundScript>,
    props: &str,
    input_state: &str,
    nonce: &str,
    events: &Sender<StreamEvent>,
) -> Result<(), Error> {
    v8::scope!(let scope, runtime);
    let framework = v8::Context::new(scope, Default::default());
    let scheduler = FrameworkScheduler::attach(framework, nonce);
    let scope = &mut v8::ContextScope::new(scope, framework);
    v8::tc_scope!(let scope, scope);
    let global = framework.global(scope);
    let app = property(scope, global, "__ssrApp")?;
    if !app.is_function() {
        return Err(Error::InvalidBundle("React application function required"));
    }
    let props = engine::parse(scope, props).ok_or_else(|| caught!(scope))?;
    let input_state = engine::parse(scope, input_state).ok_or_else(|| caught!(scope))?;
    let nonce = v8::String::new(scope, nonce)
        .ok_or(Error::InvalidResult("nonce exceeds V8 string limit"))?;
    let shell = call(
        scope,
        global,
        "render",
        &[app, props, input_state, nonce.into()],
    )?;
    let shell = finish_promise(scope, framework, &scheduler, shell)?;
    let shell = v8::Local::<v8::Object>::try_from(shell)
        .map_err(|_| Error::InvalidResult("React shell object required"))?;
    let stream = property(scope, shell, "stream")?;
    let stream = v8::Local::<v8::Object>::try_from(stream)
        .map_err(|_| Error::InvalidResult("React ReadableStream required"))?;
    let state_value = property(scope, shell, "state")?;
    let state = state(scope, framework, serializer, state_value)?;
    let reader = call(scope, stream, "getReader", &[])?;
    let reader = v8::Local::<v8::Object>::try_from(reader)
        .map_err(|_| Error::InvalidResult("React stream reader required"))?;
    let heap_used_bytes = scope.get_heap_statistics().used_heap_size();
    if events
        .send(StreamEvent::Shell(state, heap_used_bytes))
        .is_err()
    {
        return Err(Error::InvalidResult("stream receiver closed before shell"));
    }
    loop {
        let read = call(scope, reader, "read", &[])?;
        let read = finish_promise(scope, framework, &scheduler, read)?;
        let read = v8::Local::<v8::Object>::try_from(read)
            .map_err(|_| Error::InvalidResult("React stream read object required"))?;
        let done = property(scope, read, "done")?;
        if !done.is_boolean() {
            return Err(Error::InvalidResult("React stream done must be a boolean"));
        }
        if done.boolean_value(scope) {
            events
                .send(StreamEvent::End)
                .map_err(|_| Error::InvalidResult("stream receiver closed"))?;
            return Ok(());
        }
        let value = property(scope, read, "value")?;
        let bytes = v8::Local::<v8::Uint8Array>::try_from(value)
            .map_err(|_| Error::InvalidResult("React stream chunk must be Uint8Array"))?;
        let backing = bytes.get_backing_store().ok_or(Error::InvalidResult(
            "React stream chunk has no backing store",
        ))?;
        let start = bytes.byte_offset();
        let end = start
            .checked_add(bytes.byte_length())
            .filter(|end| *end <= backing.byte_length())
            .ok_or(Error::InvalidResult(
                "React stream chunk exceeds backing store",
            ))?;
        let chunk = backing[start..end]
            .iter()
            .map(std::cell::Cell::get)
            .collect();
        events
            .send(StreamEvent::Chunk(chunk))
            .map_err(|_| Error::InvalidResult("stream receiver closed"))?;
    }
}
