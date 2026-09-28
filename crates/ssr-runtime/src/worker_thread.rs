use crate::pool::Health;
use crossbeam_channel::{Receiver, bounded};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

type Thread = (JoinHandle<()>, Receiver<()>);

pub(crate) fn spawn(
    name: String,
    health: Arc<Health>,
    run: impl FnOnce() + Send + 'static,
) -> std::io::Result<Thread> {
    let (done, completed) = bounded(0);
    let thread = thread::Builder::new().name(name).spawn(move || {
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(run));
        drop(done);
        if let Err(panic) = outcome {
            health.fail(2);
            std::panic::resume_unwind(panic);
        }
    })?;
    Ok((thread, completed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Error, Pool, PoolOptions, ServerBundle};
    use std::time::Duration;

    fn verify_unwind(name: &str) {
        let pool = Arc::new(
            Pool::new(
                ServerBundle {
                    entry_path: "server/entry.js".into(),
                    entry_bytes:
                        b"export function render(props, state) {return {head:'',html:'',state};}"
                            .to_vec(),
                    chunks: Vec::new(),
                },
                PoolOptions {
                    worker_count: 1,
                    queue_capacity: 1,
                    timeout: Duration::from_secs(2),
                    cleanup_timeout: Duration::from_secs(1),
                    max_input_bytes: 1024,
                    max_queue_bytes: 1024,
                    max_chunk_bytes: 1024,
                    max_output_bytes: 1024,
                    max_heap_bytes: 134_217_728,
                },
            )
            .unwrap(),
        );
        let (waiting_tx, waiting_rx) = bounded(1);
        let waiting_pool = pool.clone();
        let waiter = thread::spawn(move || waiting_tx.send(waiting_pool.wait()).unwrap());
        let (ready, initialized) = bounded(0);
        let (worker, _completed) = spawn(name.into(), pool.health.clone(), move || {
            ready.send(()).unwrap();
            panic!("render thread failed after initialization");
        })
        .unwrap();
        initialized.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(worker.join().is_err());
        let notification = waiting_rx.recv_timeout(Duration::from_millis(200));
        pool.close().unwrap();
        waiter.join().unwrap();
        assert!(matches!(notification, Ok(Err(Error::WorkerStopped))));
    }

    #[test]
    fn worker_thread_unwind_notifies_pool_wait_without_another_render() {
        verify_unwind("ssr-runtime-0");
    }

    #[test]
    fn monitor_thread_unwind_notifies_pool_wait_without_another_render() {
        verify_unwind("ssr-runtime-watch-0");
    }
}
