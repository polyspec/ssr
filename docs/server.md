[Korean](server.ko.md)

# HTTP server

`Server::new` accepts a build, adapter selection and explicit runtime `PoolOptions`. It verifies the public files, every server JavaScript file's bytes and SHA-256, and each
file's matching private source map before creating a pool. A missing, duplicate, changed or invalid
server source map is an error. A JavaScript stack frame uses the source map for the exact server
file path in that frame.
The server uses the build's client and style URLs to create the selected document adapter.
`Adapter::React` selects the React framework and IIFE application stream path. It requires the
private framework bundle and rejects server chunks. `Adapter::Vanilla` selects the ECMAScript
module server bundle and evaluates every listed server chunk. `Adapter::Svelte` uses the same ESM
bundle and inserts Svelte head output in the document head. Both reject a React framework bundle.
All adapters use the same HTTP route and page result. The public route serves generated component
CSS from the build manifest.

`Server::handle` accepts an HTTP request with a byte body. `POST /_render` requires
`Content-Type: application/json`, optionally with `charset=utf-8`, and the exact page JSON
contract. A query is an error. React SSR returns an HTML stream after its shell is ready; status
and headers are fixed at that point. CSR returns a complete document with the unchanged input
state. A wrong method returns 405 with `Allow: POST` and no body for HEAD, a wrong content type
returns 415, and an invalid page returns 400. The existing public file route handles other paths.
Dynamic responses have `Cache-Control: no-store`. Complete responses have a byte-accurate content
length; stream responses do not declare a content length.

An exhausted pool or unavailable worker returns 503; a render timeout returns 504; a render or
JavaScript error before the React shell returns 500 with an error document. After the shell,
React sends its Suspense fallback and client recovery instructions. The server records each React
`onError` callback in a structured tracing event with the request nonce and any component stack
provided by React. A later transport or render failure ends the response body with an explicit
stream error while its status and headers stay fixed. If a stream reader rejects after sending a
chunk, the body yields that chunk before returning the JavaScript rejection as a body error. The
body does not append the document suffix after that failure.
Each request creates one nonce from operating-system random bytes and applies it to every inline
script and style. Random failure is an error. JavaScript errors before the shell include their
message and source-mapped stack in the response and tracing event. A stack
mapping failure is reported with its cause and original stack. Server construction fails if the
source map cannot be decoded. Successful SSR renders record render time, pool wait and live V8
heap bytes in a tracing event. CSR records render time without a pool measurement.

Acceptance: HTTP tests cover SSR shell timing, late errors, nonce rewriting across chunk
boundaries, CSR, public files, invalid input and method responses. A reader rejection after the
first body chunk keeps the initial status and headers and returns an explicit body error.
The JavaScript failure tests check a source location from a generated build map and a fixed map,
and the metrics test checks the tracing fields. The build test verifies private deterministic
maps and their digests.

## Development mode

`ProcessOptions` requires `ready_timeout`, `drain_timeout`, `restart_limit`, `event_capacity` and
`max_probe_bytes`. Both durations and both capacities must be positive; no value is supplied implicitly.
`ProcessOptions::validate` checks these requirements before process startup.
`event_capacity` bounds each source-event and change-report queue. A full or closed queue records the
rejected event and cause, fails new requests, stops supervision and reports the failure on close.
Source paths and output exclusions are checked before an event enters the source queue.

`Development::start` accepts absolute source paths, explicitly excluded paths, a build `Command`,
a function that creates a render `Command` from the completed build directory and owned socket path, an SSR `Page` for
preparation and explicit `ProcessOptions`. Paths contain no symbolic links.
Source directories are watched recursively. A source file uses a nonrecursive watch of its parent
with exact path filtering so replacing that file preserves observation. Excluded paths must be
strict descendants of a source path; a missing excluded path requires a verified existing ancestor.
Only source changes with an included input path enter the queue; an event with unknown paths also
enters it. Access events do not rebuild. New source files remain watched.

Both commands execute Rust programs. The build command writes only the absolute immutable directory
returned by `Build::write` and a newline to stdout; diagnostics use stderr. The render command
serves its supplied completed directory through the supplied private Unix socket and writes that
absolute socket path and a newline to stdout after its listener starts. It emits no further stdout. It stops accepting requests and
finishes active responses when stdin closes. The supervisor retains child stdin independently of
process exit observation. Build output must be outside watched inputs or under an explicit exclusion.

Before publishing a render process, the supervisor validates its complete directory with
`Build::read` and sends the supplied SSR page to `/_render`. Preparation requires HTTP 200, HTML
content type and a complete nonempty body within `max_probe_bytes`. A listening port alone is insufficient.
The preparation timeout covers startup and the entire probe response. `Development::from_build`
accepts an already completed directory with the same render command, page and options; it has no
source watcher or build command.

`Development::handle` forwards requests asynchronously, preserves the application Host header and
returns a Hyper response body. The URI authority selects the child connection address. Its
process remains running until that body completes or is dropped. After replacement, existing
responses may finish and new requests use the prepared process. A rebuild or watch failure makes
new requests fail with its cause until a later successful rebuild. The change receiver reports
each result. An invalid initial build fails startup. Unexpected process exit triggers an error
event and a replacement from the same verified build without rebuilding source, up to `restart_limit`.
Zero disables automatic replacement. Each completed build has one replacement budget; a successful
source rebuild resets it. A failed replacement or exhausted budget leaves requests failing. HTTP transport errors preserve their underlying cause.

A dedicated supervisor thread owns its Tokio runtime. `Development::close` stops the watcher and
active build, closes child stdin and collects every child exit status. A process that exceeds its
shutdown timeout is killed and waited for, and shutdown returns an error. Dropping the supervisor
also waits for cleanup and logs its result. `close` accepts a shared reference; concurrent and repeated
calls await the same completion result. The shutdown duration starts when a process is replaced,
invalidated by a failed rebuild or requested to stop, even when a response has not finished.
Extra stdout remains an error during shutdown. Build execution reports start, completion, exit failure
and elapsed time and inherits stderr; there is no total build timeout.

Acceptance: maintained Rust child programs verify separate build and render processes, actual SSR
preparation, explicit rebuild failure and recovery, progressive responses during replacement,
restart from a completed build, normal shutdown, forced termination, and process cleanup on drop.
The source event tests preserve newly created source files and exclude only declared output paths.

`POST /_render` rejects a body larger than `max_input_bytes` with HTTP 413 before JSON parsing,
including CSR requests. The HTTP transport must apply the same limit while receiving the body.

Runtime input byte exhaustion returns HTTP 413; waiting-count or waiting-byte exhaustion returns
HTTP 503. A cleanup failure that makes the pool unavailable also returns HTTP 503 before headers
are sent. Output-limit failure remains an explicit render or response-body error.

`Server::health`, `Server::wait` and `Server::close` expose the owned pool's availability, closure
event and explicit shutdown result. A renderer process uses the closure event to stop accepting
requests when native work cannot be cleaned up.

Supervised parent and child requests use a private Unix socket. Service DNS names remain external request addresses; process readiness does not resolve a service address. A request preserves its supplied Host header; a request without that header uses the internal HTTP authority `ssr-server`.

The supervisor must allocate and own the private socket directory before process startup and supply the exact socket path to the render command factory. The child declaration must match that path. A declared path alone does not authorize removal; the supervisor removes only its own socket and directory after collecting process exit. The supervisor rejects a socket path longer than a Unix socket address before it creates the directory. Readiness fails for a declared path that differs from the owned path, including a network address or a relative path, for a declared path that is not a socket, for a socket directory open to other users and for a socket directory with another entry; the supervisor keeps an entry that it did not create and reports that its directory could not be removed.
