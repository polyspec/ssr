pub fn options(
    worker_count: usize,
    queue_capacity: usize,
    timeout: std::time::Duration,
) -> polyspec_ssr_runtime::PoolOptions {
    polyspec_ssr_runtime::PoolOptions {
        worker_count,
        queue_capacity,
        timeout,
        cleanup_timeout: std::time::Duration::from_secs(2),
        max_input_bytes: 16777216,
        max_queue_bytes: 67108864,
        max_chunk_bytes: 65536,
        max_output_bytes: 67108864,
        max_heap_bytes: 134217728,
    }
}
