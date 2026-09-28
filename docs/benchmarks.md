# Render benchmark

`make bench` renders one fixed SSR page through a 16-worker snapshot runtime. Each call checks
the HTML and state. The command runs 128 calls per caller at concurrency 1, 4 and 16 and fails
on a render error, a missing sample, a changed result or a limit violation. The build reports its Cargo command and compiler progress without a total duration limit;
running the benchmark has a 120-second timeout. The measured duration starts
before `Pool::render_with_metrics` and ends
after its return. Throughput divides all completed calls by the elapsed time for that scenario;
p50 and p99 use the nearest rank of every call's duration. Context reset time surrounds V8
context creation. Memory is the change in live V8 heap bytes immediately around that creation,
not process resident memory. The command reports the largest observed change.

The following output was measured on macOS 26.6.2, Apple M3 Pro, 36 GiB memory, Rust 1.98.1,
using the Cargo development profile with debug information disabled and the verified official
V8 150.4.0 simdutf release archive on 2026-09-28. The benchmark includes worker communication,
JSON parsing and result validation in call latency; it does not include bundle compilation or
HTTP transport.

| Concurrent calls | Calls | Renders/s | p50 ms | p99 ms | Reset p50 ms | Reset p99 ms | Peak heap change bytes |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 128 | 5,252.3 | 0.185 | 0.251 | 0.163 | 0.186 | 202,112 |
| 4 | 512 | 18,088.5 | 0.199 | 0.596 | 0.176 | 0.565 | 202,064 |
| 16 | 2,048 | 29,644.3 | 0.472 | 1.628 | 0.189 | 1.159 | 204,800 |

The maintained benchmark enforces these limits for each scenario. The limits allow concurrent
host work while still failing for a substantial regression from the measured run.

| Concurrent calls | Minimum renders/s | Maximum p99 ms | Maximum reset p99 ms | Maximum peak heap change bytes |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 2,000 | 2 | 2 | 1,000,000 |
| 4 | 7,000 | 3 | 3 | 1,000,000 |
| 16 | 10,000 | 10 | 10 | 1,000,000 |
