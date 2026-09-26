[Korean](react.ko.md)

# React adapter

`ssr-adapter-react` provides `server_entry` and `client_entry` source for one application
component at an absolute path. The consumer writes these sources as TSX files under its
application root and passes their absolute paths to `ssr-build`. The adapter depends on
`ssr-core` and `ssr-runtime`; the caller passes the public client and style URLs from the
build manifest to `ReactAdapter::new` and the server entry bytes to `Pool::new`.

The server entry calls the application with props and `renderState` containing the input
state and an output field initialized to null. It returns React HTML and the output state.
The server result also contains a required empty `head` string. The adapter rejects a nonempty
`head` because it has no document placement rule for that output.
`ReactAdapter::render` inserts that HTML and output state into the SSR document. For CSR it
leaves the root empty, inserts the unchanged input state, and does not call the runtime pool.
Both modes load the same client URL. The client hydrates a nonempty root and renders an empty
root. The application uses the state embedded in the document as `renderState.input`.

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
