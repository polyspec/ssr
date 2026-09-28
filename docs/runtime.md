[Korean](runtime.ko.md)

# Runtime

`Pool::new` accepts `ServerBundle` with `entry_path`, `entry_bytes` and `chunks` containing exact
private server paths and bytes, and explicit `PoolOptions`. It evaluates the entry as a V8 module in
a snapshot, and each worker owns one isolate created from that snapshot on one thread. The entry
exports `render(props, state)`; the function is kept in context data and is absent from the global
object. Every supplied server file is compiled and its static imports are resolved during pool
construction. Dynamic chunks are evaluated when imported. Static and dynamic relative imports
resolve only to supplied server files. A missing or
duplicate file, invalid path, import attribute, failed evaluation or unfinished top-level await is
an error. A JavaScript syntax failure returns the V8 `SyntaxError` diagnostic as a JavaScript error.
A server file path begins with `server/`, ends with `.js`, and has no empty or dot
component, backslash, colon or control character. Import specifiers are relative paths without
query or fragment syntax. Valid static import cycles follow V8 module evaluation. Every file uses its manifest
path as its JavaScript source name, so its own source map resolves stack locations. Render returns
an object with required string `html` and `head`
values and a JSON `state` value, directly or through a Promise. A missing or invalid head is an
error; its UTF-8 bytes remain in `RenderResult`. The runtime completes queued V8 microtasks
before reading a Promise result. It returns a JavaScript error with the rejection message
and stack for a rejected Promise; a Promise still pending after its microtasks complete is
an error because no external event source can complete it. An absent function, invalid
result or JavaScript exception is also an error. A caller passes an SSR `Page` and
receives `RenderResult`.
The library workspace selects the same V8 150.4.0 source checkout verified by
S-4-2 and pins its maintained identifier macro dependency in `Cargo.lock`.

The snapshot key contains the ordered server file paths and bytes SHA-256, the pinned deno_core
version and the library version. The process selects one key when initialization succeeds and
retains that snapshot until process exit. Equal keys reuse the same bytes; another key returns
an explicit error even after all pools close. Production build commands and render servers use separate Rust processes.
A new application version starts in a new renderer process.

`ssr_core::process::build` protects the Svelte compiler from V8 creation through runtime disposal.
`ssr_core::process::render` protects snapshot creation through creator disposal. Concurrent compiler
operations are allowed; a snapshot operation rejects concurrent engine entry. Once a snapshot
succeeds, compiler entry fails before V8 construction. Ordinary Rust bundling remains available.
A completed Svelte build may precede rendering in the same process. Failed snapshot initialization
disposes its last isolate before releasing access, so subsequent compilation and rendering may retry.
Lock contention and poisoned locks return errors; they do not wait or bypass the process contract.

The public V8 snapshot creator evaluates the framework and application once, serializes the
context and is completely disposed before worker creation. Workers restore the same snapshot
concurrently through the public isolate API and verified official local archive. Each call
restores a separate context without running either bundle again. Initialized globals and Web
APIs are available in each context; request mutations do not affect later calls. Initialization
failure returns its error without selecting a process key and obeys the configured timeout.
Props and input state enter as V8 values. HTML, head and output state leave as bytes; the
output state is parsed through ordered-json. Undefined, function, symbol, bigint
and non-finite number values in the output state fail instead of being omitted
or converted to null. The runtime reports malformed input or output instead of
substituting a default.

`PoolOptions` declares `worker_count`, `queue_capacity`, `timeout`, `cleanup_timeout`,
`max_input_bytes`, `max_queue_bytes`, `max_chunk_bytes`, `max_output_bytes` and `max_heap_bytes`.
All limits except queue count must be positive; a zero queue count permits zero queue bytes.
`validate` rejects invalid limits before creating a snapshot or worker. Input bytes include compact
props, input state and the nonce. Waiting bytes count serialized requests waiting for a worker.
Output bytes include the output state and every HTML chunk; ordinary renders also include head.
The heap limit configures V8 heap allocation. It does not limit all native allocations or contain a
fatal V8 allocation failure; the process manager owns the process memory limit and replacement.

The pool admits at most its configured waiting calls after all workers are busy. A further call
returns `QueueFull`; exceeding a byte limit returns `LimitExceeded` without truncation.
`render_with_metrics` returns measured pool wait, context creation duration, context heap change
and the V8 used heap after rendering. `render` returns the same result without those metrics.
`Pool::health` returns the current availability; `Pool::wait` blocks on the closure event and
returns the worker failure without polling. `Pool::close` stops admission, cancels requests and
joins its threads within `cleanup_timeout`. A worker that cannot finish makes the whole pool
unavailable and requires process replacement; no replacement worker thread is created.
If initialization and worker cleanup both fail, the returned error preserves both causes.

Restored contexts expose `console` methods that write diagnostics to stderr and
`crypto.getRandomValues` backed by operating system randomness. The latter accepts
integer typed-array views of at most 65,536 bytes and writes only the view. Invalid
arguments, non-integer views, and excess lengths throw errors. The two Web Crypto
DOMException names and codes used by these errors are provided; unrelated DOM APIs
are absent. An OS failure does not modify the view. `fetch`, file and network
access, and I/O timers are absent.
The source bundle is trusted application code and does not gain process isolation.

Every render context also exposes `TextEncoder` with UTF-8 `encoding`, `encode` and
`encodeInto`. Encoding replaces an unpaired surrogate with U+FFFD. `encodeInto`
reports UTF-16 code units read and bytes written without writing a partial UTF-8
sequence. Invalid receivers and destinations throw a TypeError. The interface
supplies the encoding required by the React server bundle without exposing file,
network or timer operations.

`Pool::new_react` accepts the private framework path and bytes, an application `ServerBundle`
with no server chunks, and `PoolOptions`. It initializes the framework and
application once in the same snapshot context, so application module initialization and rendering
use the same React instance. The framework bundle runs as a function with scheduling and Web
Streams in lexical arguments. The global `setTimeout` is removed before the application bundle
executes. Application code cannot schedule I/O timers. Each `Pool::render_stream` call restores
a separate request context, accepts a new `Cancellation`, runs the render, and returns the output state and a stream when the
shell is ready. The stream yields HTML chunks without rebuilding either bundle; a failure after
the shell is an explicit stream error. A fixture renders a top-level React class and context
with `useId` and `useContext` and checks the application timer boundary.

Acceptance: tracked tests verify snapshot reuse and key mismatch, initialized state
restoration, failed and timed-out initialization, request isolation, a nonterminating script,
queue saturation, a combined queue and execution deadline, unresponsive worker
removal and waiting-call notification, captured stderr console output, random
values and argument errors, and absence of forbidden APIs. Every test has a
timeout under `make check`; the stderr capture subprocess has its own timeout.
Promise tests cover fulfillment, rejection with a stack and pending results. The
React bundle case executes an application component with top-level React declarations.
React stream cases verify repeated request contexts, different application snapshots restored
in separate renderer processes, and preservation of a failed Suspense boundary for client recovery.

## Request cancellation and limits

One configured request duration covers pool waiting, execution and stream transmission. Stream
closure cancels that request, including a native wait to send a chunk. Cancellation and completion
share request-specific state; a completed request cannot terminate an isolate serving another
request. Capacity returns only after context disposal and termination reset. Pool health reports
worker failures and pool close cancels active calls and reports thread cleanup failure within its
configured cleanup duration. A native call that cannot stop requires process replacement; the pool
does not create replacement threads. Explicit limits bound serialized input, waiting input bytes,
chunk bytes and total output bytes. Large V8 chunks are split before copying so every transmitted
chunk fits `max_chunk_bytes` and preserves the complete ordered output. Exceeding the input, queue
or total output limit returns an error and never truncates output.

Create `Cancellation::new()` before starting a stream request and pass a clone to
`render_stream(page, nonce, cancellation)`. `cancel()` interrupts waiting and running work.
A token belongs to one request; reuse is an error. `Stream::cancel_handle` clones the same token.
Dropping a stream requests cancellation without blocking the caller. A fixed monitor thread per
worker observes cancellation, expiration and cleanup completion even when no reader calls `next`.
`Stream::close` requests cancellation and reports whether cleanup completed; errors preserve both
the request failure and a cleanup failure. Cleanup clears execution termination and releases the
worker before completion is acknowledged. A completed token cannot terminate the next request.

Runtime tracing records the request nonce and worker number at execution start, cancellation
observation and cleanup completion, with elapsed time. A render failure is recorded before its
terminal event is sent, so a disconnected receiver cannot suppress the failure diagnostic.

When a request expires or is canceled after reserving a worker but before that worker receives
the command, only that request fails and its reserved capacity returns. A disconnected worker
command receiver makes the pool unavailable; it is never returned to available capacity.

If the native worker or its monitor unwinds after initialization, its thread cleanup marks the
pool unavailable and wakes `Pool::wait` without requiring another request or body read.
