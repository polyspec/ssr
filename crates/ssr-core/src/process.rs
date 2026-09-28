//! Coordinate the compiler and snapshot engine entries within one process.

use std::sync::{RwLock, TryLockError};

static RENDERED: RwLock<bool> = RwLock::new(false);

/// Run one Svelte compiler operation through disposal of its V8 runtime.
/// Concurrent compilers are allowed. Snapshot initialization and an already
/// selected renderer snapshot reject compiler entry before V8 is entered.
pub fn build<T>(compile: impl FnOnce() -> T) -> Result<T, &'static str> {
    let selected = RENDERED.try_read().map_err(|error| match error {
        TryLockError::WouldBlock => {
            "Svelte compilation cannot run while renderer initialization is active"
        }
        TryLockError::Poisoned(_) => "process engine lock is poisoned",
    })?;
    if *selected {
        return Err("Svelte compilation cannot run after renderer initialization");
    }
    Ok(compile())
}

/// Run snapshot creation through disposal of its V8 creator.
/// The operation must dispose every isolate before returning an error. A
/// successful operation permanently excludes compiler entry in this process.
pub fn render<T, E>(
    initialize: impl FnOnce() -> Result<T, E>,
) -> Result<Result<T, E>, &'static str> {
    let mut selected = RENDERED.try_write().map_err(|error| match error {
        TryLockError::WouldBlock => {
            "renderer initialization cannot run while another engine operation is active"
        }
        TryLockError::Poisoned(_) => "process engine lock is poisoned",
    })?;
    if *selected {
        return Err("renderer process already initialized a snapshot");
    }
    let result = initialize();
    if result.is_ok() {
        *selected = true;
    }
    Ok(result)
}
