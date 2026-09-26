use ssr_core::Page;
use ssr_runtime::{Pool, RenderMetrics, ServerBundle};
use std::error::Error;
use std::sync::Arc;
use std::time::{Duration, Instant};

const BUNDLE: &str = "export function render(props, state) { return {head:'', html: '<main>' + props.text + '</main>', state}; }";
const PAGE: &str = r#"{"render":"ssr","title":"Benchmark","language":"en","props":{"text":"fixed page"},"state":{"value":1}}"#;
const CALLS_PER_THREAD: usize = 128;
const CONCURRENCY: [usize; 3] = [1, 4, 16];
const MIN_RENDERS_PER_SECOND: [f64; 3] = [2000.0, 7000.0, 10000.0];
const MAX_P99: [Duration; 3] = [
    Duration::from_millis(2),
    Duration::from_millis(3),
    Duration::from_millis(10),
];
const MAX_CONTEXT_RESET_P99: [Duration; 3] = MAX_P99;
const MAX_CONTEXT_HEAP_DELTA_BYTES: [i128; 3] = [1_000_000, 1_000_000, 1_000_000];

#[derive(Debug)]
struct Summary {
    renders_per_second: f64,
    p50: Duration,
    p99: Duration,
}

fn summarize(
    results: Vec<Result<Duration, String>>,
    expected: usize,
    elapsed: Duration,
) -> Result<Summary, String> {
    if expected == 0 || results.len() != expected || elapsed.is_zero() {
        return Err("invalid benchmark sample count or elapsed time".to_owned());
    }
    let mut values = results.into_iter().collect::<Result<Vec<_>, _>>()?;
    values.sort_unstable();
    let rank = |percent: usize| (expected * percent).div_ceil(100) - 1;
    Ok(Summary {
        renders_per_second: expected as f64 / elapsed.as_secs_f64(),
        p50: values[rank(50)],
        p99: values[rank(99)],
    })
}

fn check_limits(
    index: usize,
    latency: &Summary,
    reset: &Summary,
    heap_peak: i128,
) -> Result<(), String> {
    if latency.renders_per_second < MIN_RENDERS_PER_SECOND[index]
        || latency.p99 > MAX_P99[index]
        || reset.p99 > MAX_CONTEXT_RESET_P99[index]
        || heap_peak > MAX_CONTEXT_HEAP_DELTA_BYTES[index]
    {
        return Err(format!(
            "concurrency={} exceeded recorded benchmark limits",
            CONCURRENCY[index]
        ));
    }
    Ok(())
}

fn run(index: usize, pool: Arc<Pool>, page: &Page) -> Result<(), Box<dyn Error>> {
    let concurrency = CONCURRENCY[index];
    let started = Instant::now();
    let mut samples = Vec::with_capacity(concurrency * CALLS_PER_THREAD);
    std::thread::scope(|scope| {
        let handles = (0..concurrency)
            .map(|_| {
                let pool = Arc::clone(&pool);
                scope.spawn(move || {
                    (0..CALLS_PER_THREAD)
                        .map(|_| {
                            let began = Instant::now();
                            let result = pool.render_with_metrics(page);
                            let elapsed = began.elapsed();
                            result.map_err(|error| error.to_string()).and_then(
                                |(rendered, metrics)| {
                                    if rendered.html != b"<main>fixed page</main>"
                                        || rendered.state.compact() != r#"{"value":1}"#
                                    {
                                        return Err("render output changed".to_owned());
                                    }
                                    Ok((elapsed, metrics))
                                },
                            )
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect::<Vec<_>>();
        for handle in handles {
            samples.extend(handle.join().expect("benchmark worker panicked"));
        }
    });
    let elapsed = started.elapsed();
    let count = concurrency * CALLS_PER_THREAD;
    let mut latencies = Vec::with_capacity(count);
    let mut resets = Vec::with_capacity(count);
    let mut heap_peak = i128::MIN;
    for sample in samples {
        let (
            latency,
            RenderMetrics {
                context_reset,
                context_heap_delta_bytes,
                ..
            },
        ) = sample?;
        latencies.push(Ok(latency));
        resets.push(Ok(context_reset));
        heap_peak = heap_peak.max(context_heap_delta_bytes);
    }
    let latency = summarize(latencies, count, elapsed)?;
    let reset = summarize(resets, count, elapsed)?;
    println!(
        "concurrency={concurrency} calls={count} renders/s={:.1} p50_ms={:.3} p99_ms={:.3} context_reset_p50_ms={:.3} context_reset_p99_ms={:.3} context_heap_delta_peak_bytes={heap_peak}",
        latency.renders_per_second,
        latency.p50.as_secs_f64() * 1000.0,
        latency.p99.as_secs_f64() * 1000.0,
        reset.p50.as_secs_f64() * 1000.0,
        reset.p99.as_secs_f64() * 1000.0,
    );
    check_limits(index, &latency, &reset, heap_peak)?;
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let page = Page::from_json(PAGE.as_bytes())?;
    let pool = Arc::new(Pool::new(
        ServerBundle {
            entry_path: "server/bench.js".into(),
            entry_bytes: BUNDLE.as_bytes().to_vec(),
            chunks: Vec::new(),
        },
        16,
        16,
        Duration::from_secs(10),
    )?);
    for index in 0..CONCURRENCY.len() {
        run(index, Arc::clone(&pool), &page)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_rank_uses_every_sample() {
        let values = [1, 2, 3, 4, 100].map(|value| Ok(Duration::from_millis(value)));
        let summary = summarize(values.into(), 5, Duration::from_secs(1)).unwrap();
        assert_eq!(summary.p50, Duration::from_millis(3));
        assert_eq!(summary.p99, Duration::from_millis(100));
        assert_eq!(summary.renders_per_second, 5.0);
    }

    #[test]
    fn missing_or_failed_calls_are_errors() {
        assert!(summarize(vec![], 1, Duration::from_secs(1)).is_err());
        assert!(
            summarize(
                vec![Ok(Duration::from_millis(1))],
                2,
                Duration::from_secs(1)
            )
            .is_err()
        );
        assert!(
            summarize(
                vec![Err("render failed".to_owned())],
                1,
                Duration::from_secs(1)
            )
            .is_err()
        );
        assert!(summarize(vec![Ok(Duration::from_millis(1))], 1, Duration::ZERO).is_err());
    }

    #[test]
    fn measured_regression_returns_error() {
        let slow = Summary {
            renders_per_second: MIN_RENDERS_PER_SECOND[0] - 1.0,
            p50: Duration::ZERO,
            p99: Duration::ZERO,
        };
        let fast = Summary {
            renders_per_second: MIN_RENDERS_PER_SECOND[0],
            p50: Duration::ZERO,
            p99: Duration::ZERO,
        };
        assert!(check_limits(0, &slow, &fast, 0).is_err());
        assert!(check_limits(0, &fast, &fast, MAX_CONTEXT_HEAP_DELTA_BYTES[0] + 1).is_err());
        let latency = Summary {
            p99: MAX_P99[0] + Duration::from_nanos(1),
            ..fast
        };
        assert!(check_limits(0, &latency, &fast, 0).is_err());
        let reset = Summary {
            p99: MAX_CONTEXT_RESET_P99[0] + Duration::from_nanos(1),
            ..fast
        };
        assert!(check_limits(0, &fast, &reset, 0).is_err());
    }
}
