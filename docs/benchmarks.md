# Render benchmark

`make bench` renders one fixed SSR page through a 16-worker snapshot runtime. Each call checks
the HTML and state. The command runs 128 calls per caller at concurrency 1, 4 and 16 and fails
on a render error, a missing sample, a changed result or a limit violation. Building has a
30-minute timeout; running the benchmark has a 120-second timeout. The measured duration starts
before `Pool::render_with_metrics` and ends
after its return. Throughput divides all completed calls by the elapsed time for that scenario;
p50 and p99 use the nearest rank of every call's duration. Context reset time surrounds V8
context creation. Memory is the change in live V8 heap bytes immediately around that creation,
not process resident memory. The command reports the largest observed change.

The following output was measured on macOS 26.6.2, Apple M3 Pro, 36 GiB memory, Rust 1.98.1,
using the Cargo development profile on 2026-09-26. The benchmark includes worker communication,
JSON parsing and result validation in call latency; it does not include bundle compilation or
HTTP transport.

| Concurrent calls | Calls | Renders/s | p50 ms | p99 ms | Reset p50 ms | Reset p99 ms | Peak heap change bytes |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 128 | 4,632.2 | 0.212 | 0.292 | 0.189 | 0.221 | 201,200 |
| 4 | 512 | 18,974.9 | 0.193 | 0.644 | 0.172 | 0.620 | 200,328 |
| 16 | 2,048 | 32,586.9 | 0.439 | 1.271 | 0.182 | 1.008 | 203,288 |

The maintained benchmark enforces these limits for each scenario. The limits allow concurrent
host work while still failing for a substantial regression from the measured run.

| Concurrent calls | Minimum renders/s | Maximum p99 ms | Maximum reset p99 ms | Maximum peak heap change bytes |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 2,000 | 2 | 2 | 1,000,000 |
| 4 | 7,000 | 3 | 3 | 1,000,000 |
| 16 | 10,000 | 10 | 10 | 1,000,000 |
