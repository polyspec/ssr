[Korean](vanilla.ko.md)

# Vanilla adapter

`ssr-adapter-vanilla` generates a server entry from one absolute application module path and a
client entry from one absolute application module path. The server module exports
`render(props, renderState)`, which returns an HTML string and sets `renderState.output`. The
client module exports `hydrate(root, props, renderState)` and `mount(root, props, renderState)`.
`ssr-build` takes the generated entries as files under the build root (`BuildConfig::root`). The server bundle bytes create a `Pool`; the public client and style URLs from the
build manifest create `VanillaAdapter`.

The server result contains a required empty `head` string. A nonempty `head` fails because this
adapter has no document placement rule for that output. For SSR, the adapter returns a document
with the server HTML and output state. For CSR, it returns an empty root and unchanged input state
without calling the pool. Both documents load the same
client URL. The client entry calls `hydrate` for a nonempty root and `mount` for an empty root.
The application owns DOM updates and event handlers. Hydration must retain the existing server
nodes; the browser case checks identity and that an attached button handler works.

`static_shell` accepts only a CSR page with empty props and null state. Its document can be served
on distinct paths. Client and style URLs must be local absolute JavaScript or CSS paths without
dot segments, queries or fragments. Titles and language values are HTML escaped. Embedded JSON
escapes `<`, `>` and `&`, so input cannot close a script element. Invalid UTF-8 server HTML is an
error. The browser case builds the generated entries, verifies SSR hydration, CSR rendering, two
shell paths and JavaScript errors. Node controls the browser only; build and render run in Rust.
