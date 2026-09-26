[Korean](runtime.ko.md)

# Runtime

`Pool::new` accepts a server bundle, a nonzero worker count, a queue capacity and a
nonzero execution timeout. It initializes the bundle in a V8 snapshot, and each worker
owns one isolate created from that snapshot on one thread. The bundle defines a synchronous
global `render(props, state)` function. It returns an object with string `html` and a JSON
`state` value. The runtime rejects an absent function, a promise, an invalid result,
or a JavaScript exception. A caller passes an SSR `Page` and receives `RenderResult`.
The library workspace selects the same V8 150.4.0 source checkout verified by
S-4-2 and pins its maintained identifier macro dependency in `Cargo.lock`.

The snapshot cache key contains the server bundle SHA-256, the pinned deno_core version
from the workspace manifest and the library package version. Equal keys reuse the snapshot
while a pool holds it; changed keys create new snapshots. The cache holds weak references,
so unused snapshot bytes are released. A failed bundle initialization or snapshot creation
returns an error, and initialization obeys the configured timeout. Each call restores a
separate V8 context from the snapshot in its worker isolate without executing the bundle
again. The initialized server globals and Web APIs are available in each context, and
changes made by one request cannot affect another.
Props and input state enter as V8 values. HTML and output state leave as bytes; the
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

Acceptance: tracked tests verify snapshot reuse and key mismatch, initialized state
restoration, failed and timed-out initialization, request isolation, a nonterminating script,
queue saturation, a combined queue and execution deadline, unresponsive worker
removal and waiting-call notification, captured stderr console output, random
values and argument errors, and absence of forbidden APIs. Every test has a
timeout under `make check`; the stderr capture subprocess has its own timeout.
