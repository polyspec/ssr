use ssr_core::Page;
use ssr_runtime::{Pool, RenderMetrics, ServerBundle};
use std::error::Error;
use std::sync::Arc;
use std::time::{Duration, Instant};

const BUNDLE: &str = "export function render(props, state) { return {head:'', html: '<main>' + props.text + '</main>', state}; }";
const PAGE: &str = r#"{"render":"ssr","title":"Benchmark","language":"en","props":{"text":"fixed page"},"state":{"value":1}}"#;
const CALLS_PER_THREAD: usize = 128;
const CONCURRENCY: [usize; 3] = [1, 4, 16];
const MIN_RENDERS_PER_SECOND: [f64; 3] = [1000.0, 1000.0, 1000.0];
const MAX_P99: [Duration; 3] = [
    Duration::from_millis(4),
    Duration::from_millis(4),
    Duration::from_millis(4),
];
const MAX_CONTEXT_HEAP_DELTA_BYTES: [i128; 3] = [1_000_000, 1_000_000, 1_000_000];

#[derive(Clone, Copy, Debug)]
struct Summary {
    renders_per_second: f64,
    p50: Duration,
    p99: Duration,
}

#[derive(Clone, Copy, Debug)]
struct Latency {
    wall: Summary,
    cpu: Summary,
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

/// The measurements of one scenario beyond their recorded limits. Performance is measured and
/// never fails the benchmark: each one is reported as a warning.
fn limit_warnings(index: usize, latency: &Latency, heap_peak: i128) -> Vec<String> {
    let concurrency = CONCURRENCY[index];
    let judged = &latency.cpu;
    let mut warnings = Vec::new();
    if judged.renders_per_second < MIN_RENDERS_PER_SECOND[index] {
        warnings.push(format!(
            "concurrency={concurrency} cpu_renders/s={:.1} is below the recorded minimum {:.1}",
            judged.renders_per_second, MIN_RENDERS_PER_SECOND[index]
        ));
    }
    if judged.p99 > MAX_P99[index] {
        warnings.push(format!(
            "concurrency={concurrency} cpu_p99_ms={:.3} is above the recorded maximum {:.3}",
            judged.p99.as_secs_f64() * 1000.0,
            MAX_P99[index].as_secs_f64() * 1000.0
        ));
    }
    if heap_peak > MAX_CONTEXT_HEAP_DELTA_BYTES[index] {
        warnings.push(format!(
            "concurrency={concurrency} context_heap_delta_peak_bytes={heap_peak} is above the recorded maximum {}",
            MAX_CONTEXT_HEAP_DELTA_BYTES[index]
        ));
    }
    warnings
}

/// The lines that report one warning: a `WARNING` line, and in GitHub Actions also a
/// `::warning::` annotation.
fn warning_lines(warning: &str, github_actions: bool) -> Vec<String> {
    let mut lines = vec![format!("WARNING {warning}")];
    if github_actions {
        lines.push(format!("::warning::{warning}"));
    }
    lines
}

struct Measured {
    count: usize,
    latency: Latency,
    reset: Summary,
    heap_peak: i128,
}

fn measure(concurrency: usize, pool: &Arc<Pool>, page: &Page) -> Result<Measured, String> {
    let started = Instant::now();
    let mut samples = Vec::with_capacity(concurrency * CALLS_PER_THREAD);
    std::thread::scope(|scope| {
        let handles = (0..concurrency)
            .map(|_| {
                let pool = Arc::clone(pool);
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
    let mut cpu_latencies = Vec::with_capacity(count);
    let mut resets = Vec::with_capacity(count);
    let mut cpu_elapsed = Duration::ZERO;
    let mut heap_peak = i128::MIN;
    for sample in samples {
        let (
            latency,
            RenderMetrics {
                context_reset,
                context_heap_delta_bytes,
                render_cpu,
                ..
            },
        ) = sample?;
        latencies.push(Ok(latency));
        cpu_latencies.push(Ok(render_cpu));
        cpu_elapsed += render_cpu;
        resets.push(Ok(context_reset));
        heap_peak = heap_peak.max(context_heap_delta_bytes);
    }
    Ok(Measured {
        count,
        latency: Latency {
            wall: summarize(latencies, count, elapsed)?,
            cpu: summarize(cpu_latencies, count, cpu_elapsed)?,
        },
        reset: summarize(resets, count, elapsed)?,
        heap_peak,
    })
}

fn run(index: usize, pool: &Arc<Pool>, page: &Page) -> Result<(), Box<dyn Error>> {
    let concurrency = CONCURRENCY[index];
    let Measured {
        count,
        latency,
        reset,
        heap_peak,
    } = measure(concurrency, pool, page)?;
    println!(
        "concurrency={concurrency} calls={count} cpu_renders/s={:.1} cpu_p50_ms={:.3} cpu_p99_ms={:.3} wall_renders/s={:.1} wall_p50_ms={:.3} wall_p99_ms={:.3} context_reset_p50_ms={:.3} context_reset_p99_ms={:.3} context_heap_delta_peak_bytes={heap_peak}",
        latency.cpu.renders_per_second,
        latency.cpu.p50.as_secs_f64() * 1000.0,
        latency.cpu.p99.as_secs_f64() * 1000.0,
        latency.wall.renders_per_second,
        latency.wall.p50.as_secs_f64() * 1000.0,
        latency.wall.p99.as_secs_f64() * 1000.0,
        reset.p50.as_secs_f64() * 1000.0,
        reset.p99.as_secs_f64() * 1000.0,
    );
    let github_actions = std::env::var_os("GITHUB_ACTIONS").is_some();
    for warning in limit_warnings(index, &latency, heap_peak) {
        for line in warning_lines(&warning, github_actions) {
            println!("{line}");
        }
    }
    Ok(())
}

fn bench_pool() -> Result<Arc<Pool>, ssr_runtime::Error> {
    Ok(Arc::new(Pool::new(
        ServerBundle {
            entry_path: "server/bench.js".into(),
            entry_bytes: BUNDLE.as_bytes().to_vec(),
            chunks: Vec::new(),
        },
        ssr_runtime::PoolOptions {
            worker_count: 16,
            queue_capacity: 16,
            timeout: Duration::from_secs(10),
            cleanup_timeout: Duration::from_secs(2),
            max_input_bytes: 16777216,
            max_queue_bytes: 67108864,
            max_chunk_bytes: 65536,
            max_output_bytes: 67108864,
            max_heap_bytes: 134217728,
        },
    )?))
}

fn main() -> Result<(), Box<dyn Error>> {
    let page = Page::from_json(PAGE.as_bytes())?;
    let pool = bench_pool()?;
    for index in 0..CONCURRENCY.len() {
        run(index, &pool, &page)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_time_of_a_call_excludes_concurrent_calls() {
        let pool = bench_pool().unwrap();
        let page = Page::from_json(PAGE.as_bytes()).unwrap();
        let alone = measure(1, &pool, &page).unwrap().latency.cpu.p50;
        let concurrent = measure(16, &pool, &page).unwrap().latency.cpu.p50;
        println!("CPU p50 alone={alone:?} concurrent={concurrent:?}");
        assert!(
            concurrent < alone * 4,
            "CPU p50 alone={alone:?} concurrent={concurrent:?}"
        );
        pool.close().unwrap();
    }

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

    const FAST: Summary = Summary {
        renders_per_second: MIN_RENDERS_PER_SECOND[0],
        p50: Duration::ZERO,
        p99: Duration::ZERO,
    };

    fn latency(cpu: Summary) -> Latency {
        Latency { wall: FAST, cpu }
    }

    #[test]
    fn measured_regression_warns_and_does_not_fail() {
        let slow = Summary {
            renders_per_second: MIN_RENDERS_PER_SECOND[0] - 1.0,
            ..FAST
        };
        assert_eq!(
            limit_warnings(0, &latency(slow), 0),
            ["concurrency=1 cpu_renders/s=999.0 is below the recorded minimum 1000.0"]
        );
        assert_eq!(
            limit_warnings(0, &latency(FAST), MAX_CONTEXT_HEAP_DELTA_BYTES[0] + 1),
            [
                "concurrency=1 context_heap_delta_peak_bytes=1000001 is above the recorded maximum 1000000"
            ]
        );
        let late = Summary {
            p99: MAX_P99[0] + Duration::from_micros(1),
            ..FAST
        };
        assert_eq!(
            limit_warnings(0, &latency(late), 0),
            ["concurrency=1 cpu_p99_ms=4.001 is above the recorded maximum 4.000"]
        );
        assert!(limit_warnings(0, &latency(FAST), 0).is_empty());
    }

    #[test]
    fn a_warning_is_annotated_in_github_actions() {
        assert_eq!(warning_lines("slow", false), ["WARNING slow"]);
        assert_eq!(
            warning_lines("slow", true),
            ["WARNING slow", "::warning::slow"]
        );
    }

    #[test]
    fn wall_clock_delay_within_cpu_limits_passes() {
        let delayed = Latency {
            wall: Summary {
                renders_per_second: MIN_RENDERS_PER_SECOND[0] / 10.0,
                p50: MAX_P99[0] * 10,
                p99: MAX_P99[0] * 10,
            },
            cpu: Summary {
                p99: MAX_P99[0],
                ..FAST
            },
        };
        assert!(limit_warnings(0, &delayed, 0).is_empty());
    }
}
