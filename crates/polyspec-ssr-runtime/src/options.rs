use crate::Error;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy)]
pub struct PoolOptions {
    pub worker_count: usize,
    pub queue_capacity: usize,
    pub timeout: Duration,
    pub cleanup_timeout: Duration,
    pub max_input_bytes: usize,
    pub max_queue_bytes: usize,
    pub max_chunk_bytes: usize,
    pub max_output_bytes: usize,
    pub max_heap_bytes: usize,
}

impl PoolOptions {
    pub fn validate(&self) -> Result<(), Error> {
        if self.worker_count == 0
            || self.timeout.is_zero()
            || self.cleanup_timeout.is_zero()
            || self.max_input_bytes == 0
            || self.max_chunk_bytes == 0
            || self.max_output_bytes == 0
            || self.max_heap_bytes == 0
            || (self.queue_capacity > 0 && self.max_queue_bytes == 0)
        {
            return Err(Error::InvalidConfiguration("required pool limit is zero"));
        }
        for duration in [self.timeout, self.cleanup_timeout] {
            Instant::now()
                .checked_add(duration)
                .ok_or(Error::InvalidConfiguration("timeout exceeds clock range"))?;
        }
        Ok(())
    }
}
