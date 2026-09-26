[Korean](runtime.ko.md)

# Runtime

`Pool::new` accepts `ServerBundle` with `entry_path`, `entry_bytes` and `chunks` containing exact
private server paths and bytes, a nonzero worker count, a queue capacity and a nonzero execution timeout. It evaluates the entry as a V8 module in
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

The snapshot cache key contains the ordered server file paths and bytes SHA-256, the pinned deno_core version
from the workspace manifest and the library package version. Equal keys reuse the snapshot
while a pool holds it; changed keys create new snapshots. The cache holds weak references,
so unused snapshot bytes are released. A failed bundle initialization or snapshot creation
returns an error, and initialization obeys the configured timeout. Each call restores a
separate V8 context from the snapshot in its worker isolate without executing the bundle
again. The initialized server globals and Web APIs are available in each context, and
changes made by one request cannot affect another.
Each snapshot owns an independent V8 isolate group. Its creator and all worker isolates use
that group, including when workers restore different application snapshots concurrently.
The release source build enables pointer compression, separate pointer cages and external code space;
an unsupported group is an error. The group remains alive until its snapshot and worker
isolates are released. Worker isolate restoration is not serialized across groups.
Props and input state enter as V8 values. HTML, head and output state leave as bytes; the
output state is parsed through ordered-json. Undefined, function, symbol, bigint
and non-finite number values in the output state fail instead of being omitted
or converted to null. The runtime reports malformed input or output instead of
substituting a default.

The pool admits at most its configured waiting calls after all workers are busy.
`render_with_metrics` returns the measured wait before worker acquisition and the V8
used heap bytes measured by that worker after a successful render. The ordinary
`render` call returns the same render result without metrics.
A further call returns `QueueFull`. One deadline covers queue waiting and script
execution. At that deadline the caller invokes V8 execution termination and allows
up to one second for the worker to finish and clear termination. A worker that
finishes returns `Timeout` and can be reused. If it does not respond, the call
returns `WorkerUnresponsive`, removes that worker from pool capacity, and never
waits indefinitely for its thread during pool destruction. V8 cannot interrupt
a blocked native callback; its thread can remain until process exit. When the
last worker is lost, waiting calls wake and receive the worker error. Worker
initialization and failure are errors, not missing output.

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
with no server chunks, and the worker and timeout limits. It initializes the framework and
application once in the same snapshot context, so application module initialization and rendering
use the same React instance. The framework bundle runs as a function with scheduling and Web
Streams in lexical arguments. The global `setTimeout` is removed before the application bundle
executes. Application code cannot schedule I/O timers. Each `Pool::render_stream` call restores
a separate request context, runs the render, and returns the output state and a stream when the
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
React stream cases verify repeated request contexts, three application snapshots restored
concurrently, and preservation of a failed Suspense boundary for client recovery.
