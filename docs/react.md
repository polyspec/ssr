[Korean](react.ko.md)

# React adapter

`ssr-adapter-react` provides `framework_entry`, `server_entry` and `client_entry` source for one
application component at an absolute path. `ssr-build` takes these sources as TSX files under the
build root (`BuildConfig::root`) by absolute path, with `react_framework_entry: Some(...)`. The adapter depends on `ssr-core` and `ssr-runtime`. The
private framework bundle and application server bundle execute in one V8 context and share one
React instance. The application server bundle has no server chunks. The caller passes the
framework path and bytes plus a `ServerBundle` containing the application path and bytes to
`Pool::new_react`; `Server::new` verifies the manifest and makes this call for HTTP use.

React SSR calls `Pool::render_stream` with an SSR page and the request nonce. It returns the
output state and a stream after the shell is ready. The application receives props and
`renderState` with the input state and an output field initialized to null. The output field is
fixed when the shell is ready; changing it later is an error. `ReactAdapter::stream_parts`
returns the document prefix and suffix, and the caller sends the stream between them. React's
Suspense fallback and client recovery instructions remain in the stream. The server applies the
same request nonce to every inline script and style in the complete document. React SSR does not
return an `html` or `head` field.

`ReactAdapter::render` handles CSR only. It leaves the root empty, inserts the unchanged input
state, and does not call the runtime pool. Both modes load the same client URL. The client
hydrates a nonempty root and renders an empty root. The application uses the state embedded in
the document as `renderState.input`.

`ReactAdapter::static_shell` accepts a CSR `Page` with empty props and null state. It returns
one document that a service can serve for different paths; the application can read the
browser path after it loads. A nonempty props object, non-null state or SSR mode is an error.
Client and style URLs must be local absolute JavaScript or CSS paths without dot segments,
queries or fragments. The title and language are HTML escaped, and JSON in script elements
escapes `<`, `>` and `&` so input cannot close a script element.

The browser test uses `playwright-core` 1.63.0 installed by `npm ci` and an installed
Google Chrome executable at `/Applications/Google Chrome.app/Contents/MacOS/Google Chrome`
on macOS or `/usr/bin/google-chrome` on Linux. Missing packages or browser executables fail
the test. Node controls the browser only; the Rust process builds and renders. The test
loads SSR, CSR, a script-element input, and the same static shell at two paths. It verifies
the SSR DOM node survives hydration, the CSR root renders, state values reach the client,
the injected script does not execute, and no browser JavaScript error occurs.
A separate browser case receives a Suspense fallback and React's client recovery instructions,
then verifies successful client retry and an application error boundary when the retry fails.
