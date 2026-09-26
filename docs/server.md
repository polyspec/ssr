[Korean](server.ko.md)

# HTTP server

`Server::new` accepts a build, adapter selection, worker count, queue capacity and render
timeout. It verifies the public files, every server JavaScript file's bytes and SHA-256, and each
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

`Development::start` builds the initial server and watches an absolute application root with
notify 8.2.0. Create, modify and remove events rebuild the server and client bundles, CSS and
public files. A successful rebuild creates a new pool and replaces the server as one value.
Requests already using the previous server may finish. New requests use the new server.
The change receiver reports every rebuild result. A rebuild or watch error makes new requests
fail with its cause until a later successful rebuild. An invalid initial build fails startup.
Dropping the development server stops the watcher and joins its worker.

Acceptance: a source change updates rendered and public output. An invalid source reports a
build failure, prevents stale output, and a valid edit restores service.
