[Korean](svelte.ko.md)

# Svelte adapter

`polyspec-ssr-build` compiles each `.svelte` source for server and client inside the Rust process with the
pinned Svelte 5.57.1 compiler. Packages are installed with `npm ci` before the build, which takes
absolute server, client and CSS entry paths under the build root (`BuildConfig::root`). Compilation needs
no Node process. Compiler warnings and errors fail the build. The compiler source map is passed to
the JavaScript bundler, and generated component CSS is processed with Lightning CSS and published
under a content-hashed public URL in the build manifest. Server and client compilation must return
equal component CSS. The build rejects missing compiler output and a mismatched result.
Svelte's AST and metadata are compiler tooling data. Its original CSS source map does not describe
the transformed CSS and is not published; the JavaScript source map is consumed by the bundler.

`polyspec-ssr-adapter-svelte` generates entries for one component at an absolute `.svelte` source path. The
server entry exports a `render` function that calls `svelte/server` with page props and
`renderState`. The caller constructs `ServerBundle` with the build manifest's server entry path
and bytes and every private server chunk path and bytes, then creates the pool. The render function
returns the body, head and output state. The adapter inserts head output inside the document head
and body output inside the root. CSR returns an empty root with unchanged input state and does not call the
runtime pool. `Adapter::Svelte` uses the HTTP render route and ESM pool. The HTTP output applies
one request nonce to inline head and body scripts and styles, and the public route serves the
component CSS recorded in the build manifest. Both modes load the same client URL. The document
records its render mode on the root element. The client calls `hydrate` for SSR, including an
empty body, and `mount` for CSR. Missing or invalid modes fail. [Svelte's server API](https://svelte.dev/docs/svelte/svelte-server)
defines the body and head outputs; [Svelte's client API](https://svelte.dev/docs/svelte/svelte)
defines hydration and mount.

`static_shell` requires CSR, empty props and null state. Client and style URLs must be local
absolute JavaScript or CSS paths without dot segments, queries or fragments. The adapter escapes
title and language as HTML and `<`, `>` and `&` in embedded JSON. Invalid UTF-8 server body or
head is an error. The browser case verifies the original server DOM node after hydration, an
empty SSR body, the button event, output state, CSR and shell pages, invalid modes, a
`<svelte:head>` meta element, and computed color
from the published component CSS. Svelte emits hydration comment markers for an empty component.
The browser case checks that such a component hydrates with no child elements. Removing its
markers produces an empty HTML body, invokes hydration, and reports a hydration mismatch rather
than selecting mount. Node controls the browser only; build and render run in Rust.
