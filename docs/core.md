[Korean](core.ko.md)

# Render contract

`ssr-core` defines the page input and render result shared by direct Rust calls and
`POST /_render`. The JSON request body is the page object with exactly five fields,
in this order when encoded: `render`, `title`, `language`, `props`, `state`. `render`
is `ssr` or `csr`; `title` and `language` are strings; `props` is an object;
`state` is any JSON value. Every field is required. Unknown fields, missing fields,
wrong types, unknown render modes and invalid JSON return errors. A repeated
decoded object key at any depth returns an error, including an escape sequence
that decodes to a key already present. `state` is never
silently replaced with null. JSON parsing and encoding use ordered-json and retain
the member order and number tokens of `props` and `state`. Page parsing uses
the ordered-json byte API that rejects duplicate keys.
The `Value` type used by `Page` and `RenderResult` is also exported by `ssr-core` so
adapters can use the page contract through their core dependency.

The render result contains HTML bytes and the output state as a JSON value. The
rendering component chooses that state. It is distinct from the input state and
is available to the browser for hydration. `ssr-core` does not set cache headers
or choose HTTP status codes; the HTTP server maps invalid requests and render
failures to its responses. `CALL_PATH` is `/_render`.

Acceptance: both [SSR fixture](../crates/ssr-core/tests/fixtures/ssr.json) and
[CSR fixture](../crates/ssr-core/tests/fixtures/csr.json) parse, encode and parse
again without changing values or object order. Invalid cases return errors. A
render result retains its HTML and output state.

The ordered-json dependency is a local unpublished Cargo package. The dependency
check excludes unpublished local packages from license decisions; registry
dependencies remain subject to the allowed license list.
