#![forbid(unsafe_code)]

mod construct;
mod dispatch;
mod engine;
mod error;
mod module;
mod options;
mod pool;
mod react_stream;
#[cfg(test)]
mod realm_tests;
mod request;
mod snapshot;
mod stream;
mod web;
mod worker;
mod worker_thread;

pub use error::Error;
pub use options::PoolOptions;
use ordered_json::Value;
pub use pool::Pool;
pub use request::Cancellation;
use ssr_core::RenderResult;
use std::time::Duration;
pub use stream::{Stream, StreamMetrics};

pub struct ServerBundle {
    pub entry_path: String,
    pub entry_bytes: Vec<u8>,
    pub chunks: Vec<(String, Vec<u8>)>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderMetrics {
    pub pool_wait: Duration,
    pub heap_used_bytes: usize,
    pub context_reset: Duration,
    pub context_heap_delta_bytes: i128,
}
struct ContextRender {
    result: RenderResult,
    context_reset: Duration,
    context_heap_delta_bytes: i128,
}
struct WorkerReply {
    result: Result<ContextRender, Error>,
    heap_used_bytes: usize,
}
pub(crate) fn state_from_json(bytes: &[u8]) -> Result<Value, Error> {
    ordered_json::parse_bytes_reject_duplicates(bytes).map_err(Error::InvalidJson)
}

#[cfg(test)]
mod cancellation_tests;

#[cfg(test)]
fn test_options(worker_count: usize, queue_capacity: usize, timeout: Duration) -> PoolOptions {
    PoolOptions {
        worker_count,
        queue_capacity,
        timeout,
        cleanup_timeout: Duration::from_secs(2),
        max_input_bytes: 16777216,
        max_queue_bytes: 67108864,
        max_chunk_bytes: 65536,
        max_output_bytes: 67108864,
        max_heap_bytes: 134217728,
    }
}
