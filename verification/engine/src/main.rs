use ssr_core::Page;
use ssr_runtime::{Pool, PoolOptions, ServerBundle};
use std::sync::{Arc, Barrier};
use std::time::Duration;

const SOURCE: &[u8] = br#"
    globalThis.initial = Array.from(crypto.getRandomValues(new Uint8Array(16))).join(',');
    globalThis.count = 0;
    export function render(props, state) {
        const html = globalThis.initial + ':' + ++globalThis.count;
        globalThis.initial = 'changed';
        return {head:'', html, state};
    }
"#;

fn main() {
    let pool = Arc::new(
        Pool::new(
            ServerBundle {
                entry_path: "server/entry.js".into(),
                entry_bytes: SOURCE.to_vec(),
                chunks: Vec::new(),
            },
            PoolOptions {
                worker_count: 4,
                queue_capacity: 16,
                timeout: Duration::from_secs(5),
                cleanup_timeout: Duration::from_secs(2),
                max_input_bytes: 16777216,
                max_queue_bytes: 67108864,
                max_chunk_bytes: 65536,
                max_output_bytes: 67108864,
                max_heap_bytes: 134217728,
            },
        )
        .expect("renderer pool with the verified archive"),
    );
    let barrier = Arc::new(Barrier::new(4));
    let results = std::thread::scope(|threads| {
        let workers: Vec<_> = (0..4)
            .map(|_| {
                let pool = Arc::clone(&pool);
                let barrier = Arc::clone(&barrier);
                threads.spawn(move || {
                    barrier.wait();
                    (0..20)
                        .map(|_| {
                            let page = Page::from_json(
                                br#"{"render":"ssr","title":"T","language":"en","props":{},"state":null}"#,
                            )
                            .unwrap();
                            pool.render(&page).unwrap().html
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        workers
            .into_iter()
            .flat_map(|worker| worker.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(results.len(), 80);
    for result in &results {
        assert_eq!(result, &results[0]);
        assert!(result.ends_with(b":1"));
        assert!(!result.starts_with(b"changed"));
    }
    println!("PASS same-snapshot parallel rendering results=80");
}
