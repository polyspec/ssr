[Korean](vue.ko.md)

# Vue adapter

`ssr-adapter-vue` generates server and client entries for one Vue component exported as the default
value from an absolute JavaScript module path. The consumer writes the entries under its
application root and passes them to `ssr-build`. The server bundle bytes create a `Pool`; the
public client and style URLs from the build manifest create `VueAdapter`. The fixture pins Vue
3.5.43. The generated server entry uses `createSSRApp` and awaits `renderToString` from
`vue/server-renderer`; the runtime completes its V8 microtasks and returns Promise rejection
details. [Vue's SSR API](https://vuejs.org/api/ssr) defines the Promise result.

The component receives page props and `renderState` with input state and output initialized to
null. The server result contains a required empty `head` string. A nonempty `head` fails because
this adapter has no document placement rule for that output. SSR returns the rendered HTML and
output state in the document. CSR returns an empty root and unchanged input state without calling
the pool. Both modes use the same client URL. The document records its render mode on the root
element. The client entry calls `createSSRApp(...).mount(root)` for SSR, including an empty body,
and `createApp(...).mount(root)` for CSR. Missing or invalid modes fail.
[Vue's SSR guide](https://vuejs.org/guide/scaling-up/ssr) defines the hydration
mount operation. The browser case checks that the SSR `main` node remains the same node after
mount and that the button handler works. It also checks empty SSR, CSR, invalid modes and the
static shell at two paths.

`static_shell` requires CSR, empty props and null state. Client and style URLs must be local
absolute JavaScript or CSS paths without dot segments, queries or fragments. The adapter escapes
title and language as HTML and `<`, `>` and `&` in embedded JSON. Invalid UTF-8 server HTML is an
error. Node controls the browser only; build and render run in Rust.
