use crate::Error;
use deno_core::v8::{self, FunctionCallbackArguments, Local, PinScope, ReturnValue, Value};

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
    let constructor = script
        .run(scope)
        .ok_or(Error::InvalidBundle("DOMException initialization failed"))?;
    let crypto = v8::Object::new(scope);
    let data = v8::Array::new_with_elements(scope, &[crypto.into(), constructor]);
    let callback = v8::Function::builder(get_random_values)
        .data(data.into())
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

fn throw_named_error(scope: &mut PinScope, data: Local<v8::Array>, name: &str, message: &str) {
    let Some(value) = data.get_index(scope, 1) else {
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
    let Ok(data) = Local::<v8::Array>::try_from(args.data()) else {
        throw_type_error(scope, "crypto callback data is invalid");
        return;
    };
    let Some(receiver) = data.get_index(scope, 0) else {
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
            data,
            "TypeMismatchError",
            "getRandomValues requires an integer typed array",
        );
        return;
    }
    let length = view.byte_length();
    if length > 65_536 {
        throw_named_error(
            scope,
            data,
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
            data,
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
