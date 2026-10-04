# Render benchmark

`make bench` renders one fixed SSR page through a 16-worker snapshot runtime. Each call checks
the HTML and state. The command runs 128 calls per caller at concurrency 1, 4 and 16 and fails
on a render error, a missing sample, a changed result or a limit violation. The build reports its Cargo command and compiler progress without a total duration limit;
running the benchmark has a 120-second timeout.

Each call is measured twice, starting before `Pool::render_with_metrics` and ending after its
return: as wall-clock time and as process CPU time (`CLOCK_PROCESS_CPUTIME_ID`). Rendering runs
on the pool worker threads, so the CPU time of the calling thread alone does not contain the
render; the process CPU time contains the caller, the worker and the V8 threads. At concurrency 1
the process CPU time of a call is the CPU time of that call; at concurrency 4 and 16 it also
contains the CPU time of the other calls that run at the same time. CPU throughput divides all
completed calls by the process CPU time of the scenario; wall-clock throughput divides them by
the elapsed time of the scenario. p50 and p99 use the nearest rank of every call. Context reset
time is the wall-clock time around V8 context creation. Memory is the change in live V8 heap
bytes immediately around that creation, not process resident memory. The command reports the
largest observed change.

The command judges only CPU throughput, CPU p99 and the peak heap change. Wall-clock throughput,
wall-clock p50 and p99 and context reset times are reported and not judged, because other work on
the host changes them without a change in the runtime.

The following output was measured on macOS 26.6.2, Apple M3 Pro, 36 GiB memory, Rust 1.98.1,
using the Cargo development profile with debug information disabled and the verified official
V8 150.4.0 simdutf release archive on 2026-10-05, while the host load average was about 150. The
benchmark includes worker communication, JSON parsing and result validation in call latency; it
does not include bundle compilation or HTTP transport.

| Concurrent calls | Calls | CPU renders/s | CPU p50 ms | CPU p99 ms | Wall renders/s | Wall p50 ms | Wall p99 ms | Reset p50 ms | Reset p99 ms | Peak heap change bytes |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 128 | 2,291.6 | 0.375 | 0.843 | 890.4 | 0.696 | 5.222 | 0.277 | 1.338 | 202,112 |
| 4 | 512 | 2,580.6 | 1.293 | 4.812 | 6,155.6 | 0.533 | 2.836 | 0.314 | 1.436 | 202,064 |
| 16 | 2,048 | 2,491.0 | 2.512 | 76.551 | 5,341.5 | 0.873 | 32.606 | 0.316 | 7.770 | 204,672 |

The maintained benchmark enforces these limits for each scenario. In the measured runs on the
loaded host, CPU throughput stayed between 1,961 and 3,951 renders/s, and CPU p99 stayed at or
below 0.94 ms, 5.75 ms and 76.6 ms for concurrency 1, 4 and 16. The limits allow that variation
while still failing for a substantial regression.

| Concurrent calls | Minimum CPU renders/s | Maximum CPU p99 ms | Maximum peak heap change bytes |
| ---: | ---: | ---: | ---: |
| 1 | 1,000 | 3 | 1,000,000 |
| 4 | 1,000 | 20 | 1,000,000 |
| 16 | 1,000 | 150 | 1,000,000 |
