# Render benchmark

`make bench` renders one fixed SSR page through a 16-worker snapshot runtime. Each call checks
the HTML and state. The command runs 128 calls per caller at concurrency 1, 4 and 16 and fails
on a render error, a missing sample or a changed result. Performance is measured and never fails it:
a measurement beyond its recorded limit prints a `WARNING` line, and in GitHub Actions also a
`::warning::` annotation. The build reports its Cargo command and compiler progress, and the run its elapsed time; both are
long operations without a time limit.

The benchmark builds `ssr-runtime` with its `bench` feature. With that feature,
`RenderMetrics::render_cpu` is the CPU time of the worker thread for one render
(`CLOCK_THREAD_CPUTIME_ID`), from the start of the context reset to the serialized result.
It does not contain the CPU time of other calls that run at the same time. The feature is
for the benchmark only and is not part of the public API.

The wall-clock time of a call starts before `Pool::render_with_metrics` and ends after its
return. CPU throughput divides all completed calls by the sum of their render CPU times;
wall-clock throughput divides them by the elapsed time of the scenario. p50 and p99 use the
nearest rank of every call. Context reset time is the wall-clock time around V8 context
creation. Memory is the change in live V8 heap bytes immediately around that creation, not
process resident memory. The command reports the largest observed change.

The command compares only CPU throughput, CPU p99 and the peak heap change with their limits. Wall-clock throughput,
wall-clock p50 and p99 and context reset times are reported and not compared, because other work on
the host changes them without a change in the runtime.

The following output was measured on macOS 26.6.2, Apple M3 Pro, 36 GiB memory, Rust 1.98.1,
using the Cargo development profile with debug information disabled and the verified official
V8 150.4.0 simdutf release archive on 2026-10-05, while the host load average was about 300. The
benchmark includes worker communication, JSON parsing and result validation in call latency; it
does not include bundle compilation or HTTP transport.

| Concurrent calls | Calls | CPU renders/s | CPU p50 ms | CPU p99 ms | Wall renders/s | Wall p50 ms | Wall p99 ms | Reset p50 ms | Reset p99 ms | Peak heap change bytes |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 128 | 2,543.1 | 0.363 | 0.700 | 1,942.7 | 0.509 | 1.445 | 0.361 | 0.743 | 202,112 |
| 4 | 512 | 3,308.0 | 0.210 | 1.039 | 6,163.6 | 0.475 | 2.827 | 0.193 | 1.166 | 202,064 |
| 16 | 2,048 | 3,443.7 | 0.218 | 0.914 | 10,923.4 | 0.535 | 10.253 | 0.227 | 1.743 | 204,960 |

The maintained benchmark warns past these limits for each scenario. In five runs on the loaded
host, CPU throughput stayed between 2,108 and 3,444 renders/s and CPU p99 stayed at or below
1.43 ms at every concurrency. The limits allow that variation while still warning for a
substantial regression.

| Concurrent calls | Minimum CPU renders/s | Maximum CPU p99 ms | Maximum peak heap change bytes |
| ---: | ---: | ---: | ---: |
| 1 | 1,000 | 4 | 1,000,000 |
| 4 | 1,000 | 4 | 1,000,000 |
| 16 | 1,000 | 4 | 1,000,000 |
