use crate::Error;
use deno_core::v8::MapFnTo;
use deno_core::v8::{self, FunctionCallbackArguments, Local, PinScope, ReturnValue, Value};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

pub(crate) struct FrameworkScheduler {
    callbacks: RefCell<VecDeque<v8::Global<v8::Function>>>,
    nonce: String,
}

impl FrameworkScheduler {
    pub(crate) fn attach(context: Local<v8::Context>, nonce: &str) -> Rc<Self> {
        let scheduler = Rc::new(Self {
            callbacks: RefCell::new(VecDeque::new()),
            nonce: nonce.to_owned(),
        });
        context.set_slot(Rc::clone(&scheduler));
        scheduler
    }

    pub(crate) fn next(&self) -> Option<v8::Global<v8::Function>> {
        self.callbacks.borrow_mut().pop_front()
    }
}

pub(crate) fn external_references() -> Vec<v8::ExternalReference> {
    vec![
        v8::ExternalReference {
            function: console_write.map_fn_to(),
        },
        v8::ExternalReference {
            function: get_random_values.map_fn_to(),
        },
        v8::ExternalReference {
            function: encode_utf8.map_fn_to(),
        },
        v8::ExternalReference {
            function: framework_schedule.map_fn_to(),
        },
        v8::ExternalReference {
            function: report_react_error.map_fn_to(),
        },
    ]
}

pub(crate) fn install_framework(
    scope: &mut PinScope,
    context: Local<v8::Context>,
) -> Result<(), Error> {
    install(scope, context)?;
    let schedule = v8::Function::builder(framework_schedule)
        .build(scope)
        .ok_or(Error::InvalidBundle(
            "React scheduling callback initialization failed",
        ))?;
    let name = v8::String::new(scope, "setTimeout")
        .ok_or(Error::InvalidBundle("React scheduling name unavailable"))?;
    if context
        .global(scope)
        .set(scope, name.into(), schedule.into())
        != Some(true)
    {
        return Err(Error::InvalidBundle(
            "React scheduling callback installation failed",
        ));
    }
    let report = v8::Function::builder(report_react_error)
        .build(scope)
        .ok_or(Error::InvalidBundle(
            "React error callback initialization failed",
        ))?;
    let name = v8::String::new(scope, "__ssrReportReactError").ok_or(Error::InvalidBundle(
        "React error callback name unavailable",
    ))?;
    if context.global(scope).set(scope, name.into(), report.into()) != Some(true) {
        return Err(Error::InvalidBundle(
            "React error callback installation failed",
        ));
    }
    Ok(())
}

fn report_react_error(scope: &mut PinScope, args: FunctionCallbackArguments, _result: ReturnValue) {
    if args.length() != 2 || !args.get(1).is_object() {
        throw_type_error(
            scope,
            "React onError requires an error and error information",
        );
        return;
    }
    let Some(message) = args.get(0).to_string(scope) else {
        throw_type_error(scope, "React error conversion failed");
        return;
    };
    let message = message.to_rust_string_lossy(scope);
    let Ok(info) = Local::<v8::Object>::try_from(args.get(1)) else {
        throw_type_error(scope, "React error information must be an object");
        return;
    };
    let Some(key) = v8::String::new(scope, "componentStack") else {
        throw_type_error(scope, "React component stack name is unavailable");
        return;
    };
    let Some(stack) = info.get(scope, key.into()) else {
        return;
    };
    let component_stack = if stack.is_undefined() {
        None
    } else if stack.is_string() {
        Some(stack.to_rust_string_lossy(scope))
    } else {
        throw_type_error(scope, "React component stack must be a string");
        return;
    };
    let context = scope.get_current_context();
    let Some(scheduler) = context.get_slot::<FrameworkScheduler>() else {
        throw_type_error(
            scope,
            "React error callback is not attached to this context",
        );
        return;
    };
    tracing::error!(nonce = %scheduler.nonce, error = %message, component_stack = ?component_stack, "React stream error");
}

fn framework_schedule(
    scope: &mut PinScope,
    args: FunctionCallbackArguments,
    mut result: ReturnValue,
) {
    let Ok(callback) = Local::<v8::Function>::try_from(args.get(0)) else {
        throw_type_error(scope, "React scheduler requires a function");
        return;
    };
    if args.length() > 2
        || (args.length() == 2
            && !args.get(1).is_undefined()
            && (!args.get(1).is_number() || args.get(1).number_value(scope) != Some(0.0)))
    {
        throw_type_error(scope, "React scheduler supports immediate tasks only");
        return;
    }
    let context = scope.get_current_context();
    let Some(scheduler) = context.get_slot::<FrameworkScheduler>() else {
        throw_type_error(scope, "React scheduler is not attached to this context");
        return;
    };
    scheduler
        .callbacks
        .borrow_mut()
        .push_back(v8::Global::new(scope, callback));
    result.set(v8::Integer::new(scope, 0).into());
}

pub(crate) fn install(scope: &mut PinScope, context: Local<v8::Context>) -> Result<(), Error> {
    let global = context.global(scope);
    let console = v8::Object::new(scope);
    for name in ["log", "info", "warn", "error", "debug"] {
        let callback =
            v8::Function::builder(console_write)
                .build(scope)
                .ok_or(Error::InvalidBundle(
                    "console callback initialization failed",
                ))?;
        let name =
            v8::String::new(scope, name).ok_or(Error::InvalidBundle("console name unavailable"))?;
        if console.set(scope, name.into(), callback.into()) != Some(true) {
            return Err(Error::InvalidBundle("console method initialization failed"));
        }
    }
    let name = v8::String::new(scope, "console")
        .ok_or(Error::InvalidBundle("console name unavailable"))?;
    if global.set(scope, name.into(), console.into()) != Some(true) {
        return Err(Error::InvalidBundle("console initialization failed"));
    }
    let source = v8::String::new(scope, include_str!("dom_exception.js"))
        .ok_or(Error::InvalidBundle("DOMException source unavailable"))?;
    let script = v8::Script::compile(scope, source, None)
        .ok_or(Error::InvalidBundle("DOMException initialization failed"))?;
    script
        .run(scope)
        .ok_or(Error::InvalidBundle("DOMException initialization failed"))?;
    let crypto = v8::Object::new(scope);
    let callback = v8::Function::builder(get_random_values)
        .build(scope)
        .ok_or(Error::InvalidBundle(
            "crypto callback initialization failed",
        ))?;
    let name = v8::String::new(scope, "getRandomValues")
        .ok_or(Error::InvalidBundle("crypto method name unavailable"))?;
    if crypto.set(scope, name.into(), callback.into()) != Some(true) {
        return Err(Error::InvalidBundle("crypto method initialization failed"));
    }
    let name =
        v8::String::new(scope, "crypto").ok_or(Error::InvalidBundle("crypto name unavailable"))?;
    if global.set(scope, name.into(), crypto.into()) != Some(true) {
        return Err(Error::InvalidBundle("crypto initialization failed"));
    }
    let encoder = v8::Function::builder(encode_utf8)
        .build(scope)
        .ok_or(Error::InvalidBundle("UTF-8 callback initialization failed"))?;
    let name = v8::String::new(scope, "__ssrEncodeUtf8")
        .ok_or(Error::InvalidBundle("UTF-8 callback name unavailable"))?;
    if global.set(scope, name.into(), encoder.into()) != Some(true) {
        return Err(Error::InvalidBundle("UTF-8 callback initialization failed"));
    }
    let source = v8::String::new(scope, include_str!("text_encoder.js"))
        .ok_or(Error::InvalidBundle("TextEncoder source unavailable"))?;
    let script = v8::Script::compile(scope, source, None)
        .ok_or(Error::InvalidBundle("TextEncoder initialization failed"))?;
    script
        .run(scope)
        .ok_or(Error::InvalidBundle("TextEncoder initialization failed"))?;
    Ok(())
}

fn encode_utf8(scope: &mut PinScope, args: FunctionCallbackArguments, mut result: ReturnValue) {
    let Some(input) = args.get(0).to_string(scope) else {
        return;
    };
    let bytes = input.to_rust_string_lossy(scope).into_bytes();
    let length = bytes.len();
    let store = v8::ArrayBuffer::new_backing_store_from_vec(bytes).make_shared();
    let buffer = v8::ArrayBuffer::with_backing_store(scope, &store);
    let Some(array) = v8::Uint8Array::new(scope, buffer, 0, length) else {
        throw_type_error(scope, "UTF-8 output exceeds typed array limit");
        return;
    };
    result.set(array.into());
}

fn console_write(scope: &mut PinScope, args: FunctionCallbackArguments, _result: ReturnValue) {
    let mut values = Vec::with_capacity(args.length() as usize);
    for i in 0..args.length() {
        let Some(value) = args.get(i).to_string(scope) else {
            throw_type_error(scope, "console argument conversion failed");
            return;
        };
        values.push(value.to_rust_string_lossy(scope));
    }
    eprintln!("{}", values.join(" "));
}

fn throw_type_error(scope: &mut PinScope, message: &str) {
    if let Some(message) = v8::String::new(scope, message) {
        let error = v8::Exception::type_error(scope, message);
        scope.throw_exception(error);
    }
}

fn throw_named_error(scope: &mut PinScope, name: &str, message: &str) {
    let context = scope.get_current_context();
    let Some(key) = v8::String::new(scope, "DOMException") else {
        throw_type_error(scope, "DOMException name is unavailable");
        return;
    };
    let Some(value) = context.global(scope).get(scope, key.into()) else {
        throw_type_error(scope, "DOMException constructor is unavailable");
        return;
    };
    let Ok(constructor) = Local::<v8::Function>::try_from(value) else {
        throw_type_error(scope, "DOMException constructor is invalid");
        return;
    };
    let Some(message) = v8::String::new(scope, message) else {
        return;
    };
    let Some(name) = v8::String::new(scope, name) else {
        return;
    };
    if let Some(error) = constructor.new_instance(scope, &[message.into(), name.into()]) {
        scope.throw_exception(error.into());
    } else {
        throw_type_error(scope, "DOMException construction failed");
    }
}

fn integer_view(value: Local<Value>) -> bool {
    value.is_int8_array()
        || value.is_uint8_array()
        || value.is_uint8_clamped_array()
        || value.is_int16_array()
        || value.is_uint16_array()
        || value.is_int32_array()
        || value.is_uint32_array()
        || value.is_big_int64_array()
        || value.is_big_uint64_array()
}

fn get_random_values(
    scope: &mut PinScope,
    args: FunctionCallbackArguments,
    mut result: ReturnValue,
) {
    let context = scope.get_current_context();
    let Some(crypto_name) = v8::String::new(scope, "crypto") else {
        throw_type_error(scope, "crypto name is unavailable");
        return;
    };
    let Some(receiver) = context.global(scope).get(scope, crypto_name.into()) else {
        throw_type_error(scope, "crypto receiver is unavailable");
        return;
    };
    if !args.this().strict_equals(receiver) {
        throw_type_error(scope, "getRandomValues requires its Crypto receiver");
        return;
    }
    let value = args.get(0);
    let Ok(view) = Local::<v8::ArrayBufferView>::try_from(value) else {
        throw_type_error(scope, "getRandomValues requires an ArrayBufferView");
        return;
    };
    let Some(backing) = view.get_backing_store() else {
        throw_type_error(scope, "getRandomValues requires a backing buffer");
        return;
    };
    if backing.is_shared() || backing.is_resizable_by_user_javascript() {
        throw_type_error(scope, "getRandomValues requires a fixed non-shared buffer");
        return;
    }
    if !integer_view(value) {
        throw_named_error(
            scope,
            "TypeMismatchError",
            "getRandomValues requires an integer typed array",
        );
        return;
    }
    let length = view.byte_length();
    if length > 65_536 {
        throw_named_error(
            scope,
            "QuotaExceededError",
            "getRandomValues exceeds 65536 bytes",
        );
        return;
    }
    let offset = view.byte_offset();
    let Some(end) = offset
        .checked_add(length)
        .filter(|end| *end <= backing.byte_length())
    else {
        throw_type_error(scope, "getRandomValues view exceeds the backing buffer");
        return;
    };
    if fill_view(&backing[offset..end], getrandom::fill).is_err() {
        throw_named_error(
            scope,
            "OperationError",
            "operating system randomness is unavailable",
        );
        return;
    }
    result.set(value);
}

fn fill_view<E>(
    cells: &[std::cell::Cell<u8>],
    fill: impl FnOnce(&mut [u8]) -> Result<(), E>,
) -> Result<(), E> {
    let mut bytes = vec![0; cells.len()];
    fill(&mut bytes)?;
    for (cell, byte) in cells.iter().zip(bytes) {
        cell.set(byte);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    #[test]
    fn failed_random_source_preserves_view() {
        let cells = [const { Cell::new(42) }; 32];
        let result = fill_view(&cells, |bytes| {
            bytes[..8].fill(99);
            Err("OS random failed")
        });
        assert_eq!(result, Err("OS random failed"));
        assert!(cells.iter().all(|cell| cell.get() == 42));
    }
}
