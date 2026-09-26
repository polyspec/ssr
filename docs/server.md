[Korean](server.ko.md)

# HTTP server

`Server::new` accepts a build, adapter selection, worker count, queue capacity and render
timeout. It verifies the public files, server JavaScript bytes and SHA-256, and the matching
private source map
before creating a pool. A missing, duplicate, changed or invalid server source map is an error.
The server uses the build's client and style URLs to create the selected document adapter.
`Adapter::React` selects the React adapter. The server keeps this selection separate from the
page contract so additional specified adapters use the same HTTP route and page result.

`Server::handle` accepts an HTTP request with a byte body. `POST /_render` requires
`Content-Type: application/json`, optionally with `charset=utf-8`, and the exact page JSON
contract. A query is an error. It returns a complete HTML
document with the output hydration state for SSR, or a CSR document with the unchanged input
state. A wrong method returns 405 with `Allow: POST` and no body for HEAD, a wrong content type returns 415, and an
invalid page returns 400. The existing public file route handles other paths. Dynamic responses
have `Cache-Control: no-store` and a byte-accurate content length.

An exhausted pool or unavailable worker returns 503; a render timeout returns 504; other render
failures return 500. JavaScript errors include their message and source-mapped stack in the
response and tracing event. A stack
mapping failure is reported with its cause and original stack. Server construction fails if the
source map cannot be decoded. Successful SSR renders record render time, pool wait and live V8
heap bytes in a tracing event. CSR records render time without a pool measurement.

Acceptance: HTTP tests cover SSR, CSR, public files, invalid input and method responses.
The JavaScript failure tests check a source location from a generated build map and a fixed map,
and the metrics test checks the tracing fields. The build test verifies private deterministic
maps and their digests.
