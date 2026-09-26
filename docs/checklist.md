[Korean](checklist.ko.md)

# Checklist

States: `[ ]` waiting, `[~]` in progress, `[o]` complete, `[!]` temporarily bypassed.
[AGENTS.md](../AGENTS.md) defines the work procedure. Each item names its dependencies and
completion evidence.

## Requirements

- One Rust process bundles and renders; no Node process runs at build time or at render time.
  `npm ci` installs packages before a build.
- A render call crosses no foreign function boundary: the caller is Rust and calls the library
  directly.
- The bundle is loaded and compiled once per V8 isolate, not sent or compared on every call.
- Props enter the isolate once per call as a V8 value; the result leaves as bytes without a second
  JSON round trip.
- A context reset restores the global state from a snapshot instead of rebuilding it.
- The first consumer needs: SSR or CSR selected per render with one client bundle, the render state
  input and output for hydration, static CSR shells, publication of the public build files into a
  directory, serving of the build files, a bounded pool with a queue, a render timeout that
  terminates a running script, and operating system random values.

## Items

- [o] S-0 Initialize the local Git repository with the author `min-median-max`, `AGENTS.md`, this
  checklist, the changelog and `.gitignore`. Depends on: none. Evidence: the first commit;
  `git remote -v` prints nothing.
- [o] S-0-1 Specify the first consumer's render, build, and engine requirements and their
  verification items. Depends on: S-0. Evidence: the requirements above and the build and engine
  acceptance criteria in S-3 and S-4 are committed.
- [o] S-0-2 Define the temporary bypass state in both language versions of the procedure and
  checklist, and include it in the checklist state check. Depends on: S-0-1. Evidence: the document
  check validates both language versions and this change is committed.
- [o] S-1 Create the Cargo workspace (`rust-toolchain.toml` with Rust 1.98.1, edition 2024) with the
  crates of [AGENTS.md](../AGENTS.md) and the Makefile targets `check` (rustfmt, clippy with
  `-D warnings`, `cargo deny check`, the record, terminology and document checks, the unit tests with
  cargo-nextest 0.9.146 and a timeout per test) and `bench`. Depends on: S-0-1. Evidence:
  `make check` exits with 0. Acceptance: the manifest has exactly the eight crates in AGENTS.md;
  every crate compiles as edition 2024; `check` fails for invalid Rust formatting, Clippy warnings,
  dependency findings, invalid records or terms, broken document pairs or links, and failed or
  timed-out tests; `bench` is an executable target.
- [o] S-1-1 Limit document and terminology checks to maintained sources. Installed package files
  under `node_modules` are dependencies and must not be checked as product records. Depends on:
  S-1. Evidence: the checker reports an invalid maintained document and ignores a package README
  in nested `node_modules`; `make check` exits with 0 after packages are installed.
- [o] S-1-2 Allow the specified dependency licenses in the dependency check. A license outside
  MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Unicode-3.0, Zlib and MPL-2.0 remains an
  error unless an exact crate exception is recorded. Depends on: S-1. Evidence: `make check` exits
  with 0; cargo-deny retains the license check.
- [o] S-1-3 Review required build dependency licenses and allow the two exact crate versions in
  `deny.toml`. All other crates and versions retain the common license policy. Depends on: S-1-2.
  Evidence: the selected versions pass the license check; another version fails it; `make check`
  exits with 0.
- [ ] S-1-4 Enforce test ownership across the workspace. For each declared shared behavior,
  run a base test in its owning crate and an actual use test in every consuming crate. Reject a
  missing, misplaced, empty, failed, or timed-out test and a declaration that only names a file.
  Depends on: S-1. Evidence: mutation cases fail for each invalid declaration, the declared tests
  execute in their own crates, and `make check` runs the ownership check.
- [o] S-2 Implement `ssr-core`: the render call `Page{render,title,language,props,state}` of
  `POST /_render` with the render mode `ssr` or `csr`, its result (HTML and the output state), and its
  errors. Cache headers are the policy of the consumer and are not part of this crate. Depends on:
  S-1. Evidence: the JSON fixtures round-trip through ordered-json. Acceptance: exactly five
  required fields; `render` is `ssr` or `csr`; `title` and `language` are strings; `props` is an
  object; `state` is any JSON value. Missing or extra fields, invalid types and invalid JSON
  return errors. See [render contract](core.md).
- [o] S-2-1 Reject duplicate decoded object keys at every depth of a page JSON request,
  including keys with equivalent escape sequences. Depends on: S-2. Evidence: tracked
  tracked duplicate-key cases fail parsing through ordered-json; unique fixtures still round-trip;
  `make check` exits with 0.
- [o] S-3 Verify the bundler: with the Rust API of rolldown 1.2.11, build a server bundle and a
  client bundle from TSX with React, code splitting and content-hashed names, and with
  lightningcss 1.0.0-alpha.72 bundle CSS with `@import` from `node_modules` and `url()` rewriting,
  and emit fonts and images (png, svg, jpg, gif, webp, avif, ico, woff, woff2, ttf) as hashed files.
  Record the result and the chosen API in `docs/build.md`; an unsupported case is written there with
  its replacement before S-5 starts. Depends on: S-1. Evidence: the sample build of each case and the
  document. Completion requires a supported CSS bundler dependency and a passing `make check`.
- [o] S-4 Verify the engine: build deno_core 0.412.0 with deno_webidl 0.259.0 and deno_web 0.290.0
  on aarch64 and x86_64 macOS and Linux, record how `deno_v8` 0.4.0 obtains its prebuilt archive,
  the variable that selects a local archive and its SHA-256. Depends on: S-1. Evidence: the builds
  and `docs/engine.md`.
- [o] S-4-1 Mount the engine verification source read only and the Cargo home and target as writable
  host directories under ignored `var/`. The host inspects the files directly, and a replacement of
  the engine container retains the cached crates and build output. Depends on: S-4. Acceptance: all
  three mounts have the stated access, `containerctl status` reads the absolute path configuration,
  the second build after container replacement reuses the linked program without compiling crates,
  and no named volume is created. Evidence: the mount and status inspections, two builds and host
  file inspection recorded in `docs/engine.md`.
- [o] S-4-2 Replace the unmaintained compile-time identifier macro dependency in the V8 source
  checkout with the maintained package, and use that checkout for engine verification builds.
  Depends on: S-4-1. Acceptance: `cargo deny check` reports no unmaintained macro dependency;
  the engine verification program fully builds and links on macOS and Linux for AArch64 and x86_64;
  the Linux AArch64 container mounts the V8 checkout read only. Evidence: the local V8 package
  commit, four target builds, mount inspection and `docs/engine.md`.
- [o] S-5 Implement `ssr-build` as decided by S-3: server and client bundles, CSS, hashed assets and
  the manifest. Depends on: S-2, S-3. Acceptance: the build accepts absolute source paths and a
  local absolute asset route, returns all generated bytes and an ordered-json manifest, and fails
  for missing entries, imports and assets, invalid routes, or conflicting output paths. The manifest
  identifies the server entry, client entry, client styles, server chunks and public assets with
  SHA-256 digests. Evidence: a sample application builds; the manifest and bytes match; the hashes
  and output bytes are stable across two builds.
- [o] S-6 Implement `ssr-runtime`: a pool of isolates with one isolate per worker thread and a
  bounded queue, the bundle compiled once per isolate, own operations for `console` and
  `crypto.getRandomValues` from the operating system, no fetch, I/O timers, files or network, a
  timeout that terminates a running script, and global state reset between requests. Depends on:
  S-2, S-4, S-4-2. Evidence: a global change of request A is absent in request B; a script that does not end
  is terminated at the timeout; a full queue returns an error; the Web API cases pass.
- [o] S-6-1 Expose the UTF-8 `TextEncoder` interface in every render context so a React server
  bundle executes. Depends on: S-6. Acceptance: construction, `encoding`, `encode` and
  `encodeInto` follow their UTF-8 behavior for ASCII, non-ASCII text, lone surrogates and a short
  destination; invalid receivers and destinations fail. The interface remains available after
  context reset and adds no file, network or timer API. Evidence: Web API tests and execution of a
  React server bundle.
- [o] S-6-2 Verify that one render deadline covers both pool wait and execution after confirming
  the request has entered the bounded queue. The queue count exposes no event, so the case uses a
  bounded yield loop to observe it. Depends on: S-6. Evidence: the synchronized timeout case and
  `make check` pass.
- [o] S-7 Implement the React adapter for the first consumer: SSR renders HTML with the render state
  output; CSR returns the static shell of the same client bundle; the client hydrates the SSR HTML
  or renders the CSR shell. Depends on: S-5, S-6, S-6-1. Acceptance: the adapter supplies server and client
  React entries for one application component and uses one client URL in both documents; SSR passes
  the input state to the component and returns and embeds its output state; CSR leaves the root empty,
  embeds the unchanged input state and does not enter the runtime pool; a static CSR shell has empty
  props and null state and can be reused for different request paths; JSON embedded in HTML cannot
  close a script element; invalid local client or style URLs and static shell values fail. Evidence:
  [React adapter](react.md) documents the contract, and a real Chrome browser verifies SSR
  hydration, CSR rendering, script input and static shell reuse at two paths.
- [o] S-8 Implement publication and serving: the public build files are written into a directory
  atomically; an existing file with equal content stays; an existing file with other content is an
  error; the build files are served with their content types. Depends on: S-5. Acceptance: only
  manifest entries with public URLs are selected; their bytes and SHA-256 digests must agree; URL
  paths must be safe and unique. Publication requires an existing absolute directory, preserves
  equal files and unrelated files, rejects different bytes and nonregular targets, and makes each
  new file visible only after its complete contents have been written. Serving handles GET and
  HEAD for exact public URLs, returns the manifest content type and bytes, returns 404 for unknown
  paths, and returns 405 for other methods. Evidence: the three publication cases, invalid input
  cases and HTTP serving cases.
- [o] S-9 Implement `ssr-server`: `POST /_render`, tracing metrics for render time, pool wait and
  heap, and stack traces mapped through source maps with sourcemap 9.3.2. Depends on: S-7, S-8.
  Acceptance: the build records private source maps for the server entry and server chunks with
  deterministic bytes and full SHA-256 digests. The runtime reports measured pool wait and live
  V8 heap bytes; the server records those values and render duration in tracing for SSR. The
  endpoint rejects an invalid method, content type, query and page, accepts a UTF-8 JSON media
  type, returns an HTML document for SSR and CSR, and returns explicit errors for render failure.
  JavaScript stack frames from the server
  script name are mapped to source locations; a missing or invalid map fails server construction.
  See the [HTTP server contract](server.md). Evidence: stable build output, runtime metrics, HTTP
  cases, mapped stack and `make check`.
- [o] S-10 Implement snapshots keyed by the server bundle hash, the deno_core version and the library
  version; a context reset restores from the snapshot; a key mismatch creates a new snapshot.
  Depends on: S-6. Acceptance: a snapshot includes initialized server globals and Web APIs;
  every request restores a separate context from the snapshot without running the bundle again;
  request changes cannot reach later requests; equal keys reuse snapshot bytes and a changed
  bundle or version creates a new snapshot; snapshot failures return errors and initialization
  obeys the configured timeout. Evidence: mismatch, reset and initialization tests and `make check`.
- [o] S-11 Implement `make bench`: renders per second and p50/p99 latency for a fixed page with 1, 4
  and 16 concurrent calls, and time and memory of a context reset; record the limits in
  `docs/benchmarks.md`. Depends on: S-9, S-10. Acceptance: run a fixed SSR page through the
  snapshot runtime with a separate measured duration for each call; report every call or fail,
  calculate nearest-rank p50/p99 and throughput from all successful calls, and measure context
  creation time and the change in live V8 heap bytes during creation. Reject invalid sample counts,
  render errors and measurements outside recorded limits. Evidence: failure and percentile cases,
  measured output and recorded limits.
- [o] S-12 Pin the V8 archive by path and SHA-256 as recorded by S-4; a failed download is an error.
  An explicit offline mode requires the existing verified archive and disables Cargo network access.
  Depends on: S-4. Evidence: missing, changed and failed-download cases and a build without network pass.
- [o] S-13 Stream React output with `renderToReadableStream`: status and headers are fixed when the
  shell is ready; a render or JavaScript error before the shell returns 500 with an error page.
  Pool exhaustion or an unavailable worker returns 503, and a render timeout returns 504. For an error inside a
  Suspense boundary after the shell, preserve its fallback and React's client recovery instructions
  in the stream, and record `onError` with the request nonce in a structured server error event;
  a client retry failure reaches the application's error boundary.
  Every inline script and style includes the
  nonce of the call. The server creates one nonce per request from operating-system random bytes;
  random failure is an error, all inline scripts and styles of that call use that nonce, and
  different calls use different nonces. React's stream and scheduling capabilities are available
  only in its framework scope; application code cannot access I/O timers. Depends on: S-7,
  S-13-1, S-13-2. Evidence: shell, late-error, nonce, timer-isolation and SSR metric cases pass.
- [o] S-13-1 Complete V8 microtasks for a Promise render result, return rejection details and
  reject a Promise that has no scheduled completion. Verify that React can render an application
  component without exposing a framework timer to that application. Depends on: S-6, S-7.
  Evidence: fulfilled, rejected and pending Promise tests, a React component test, and `make check` pass.
- [o] S-13-2 Provide isolated React scheduling and Web Streams, carry stream chunks from the V8
  worker to the HTTP response after the shell is ready, and apply the request nonce to inline
  scripts and styles. Depends on: S-13-1, S-13-2-1, S-13-2-2, S-13-2-2-3. Evidence: the S-13 cases pass
  with progressive chunks and fixed status and headers. A reader rejection after the first body
  chunk returns an explicit body error while the response keeps its initial status and headers.
- [o] S-13-2-1 Build a separate private React framework bundle and an application server bundle
  that imports the same React instance. Validate the absolute framework entry,
  manifest bytes and SHA-256, source map and absence of a public URL. Depends on: S-13-1.
  Evidence: an application component with `useId` renders without an application timer; missing
  and repeated entries fail; build tests and `make check` pass.
- [o] S-13-2-1-1 Define and verify the React bundle execution boundary with an executable V8
  fixture. The fixture evaluates the framework and application bundles in one context so module
  initialization can use `Component` and `createContext`, passes scheduling as a lexical function
  argument and removes the global timer before application evaluation. Depends on: S-13-2-1.
  Evidence: a top-level React class and context render with `useId` and `useContext`, application
  code cannot access `setTimeout`, and `make check` passes.
- [o] S-13-2-2 Execute the two bundles with isolated scheduling and Web Streams, and stream the
  document through HTTP with the nonce and error behavior of S-13. Apply the verified execution
  boundary to the production snapshot and pool; neither bundle runs on each request. Repeated
  restores keep one initialization and separate globals, and three application snapshots restore
  concurrently. The React application server bundle is an IIFE without server chunks; reject a
  React manifest that lists any. The vanilla HTTP path retains the ECMAScript module chunk and
  source-map cases. Depends on: S-13-2-1-1, S-13-2-2-1, S-13-2-2-1-1, S-13-2-2-2, S-15-1.
  Evidence: the shell, Suspense fallback, client recovery, HTTP 500/503/504, nonce, SSR metric
  and parallel restore cases pass in the runtime, server and browser.
- [o] S-13-2-2-3 Correct the React adapter and runtime documents to describe the public
  framework and application entry, `Pool::new_react`, `render_stream`, `stream_parts`, and CSR
  contracts. Remove the obsolete synchronous React render description. This correction precedes
  S-13 completion. Depends on: S-13-2-2. Evidence: a tracked contract check fails on the old
  React document and passes on the corrected English and Korean documents; the public React
  consumer test and `make check` pass.
- [o] S-13-2-2-1 Create a V8 isolate group for each server snapshot and restore every worker
  isolate in that group. Keep separate application snapshots independent when workers restore
  them concurrently; do not serialize worker isolate creation. Build the pinned V8 source with
  `is_debug=false`, separate pointer cages and external code space, and fail if those capabilities
  are unavailable.
  Depends on: S-10. Evidence: three distinct snapshots restore concurrently in repeated rounds,
  existing snapshot and benchmark cases pass, and `make check` passes.
- [o] S-13-2-2-1-1 Verify a separate absolute local V8 source archive and its generated binding
  by their recorded SHA-256 values before `make check` and `make bench` link them. Reject a missing
  or changed file and a source path equal to a Cargo output path. Keep direct source-build
  verification available as an explicit command; routine checks reuse the verified files without
  invoking GN or Ninja. Depends on: S-13-2-2-1. Evidence: missing, changed, identical-path and verified-copy cases,
  two routine checks with no V8 source rebuild, and the existing benchmark limits pass.
- [o] S-13-2-2-2 Pass the exact server entry and chunks through `ServerBundle` and evaluate
  them as V8 modules. The server entry
  exports a `render` function; it does not create a global render function. Generated server
  entries and every direct pool consumer use this contract. Resolve static and
  dynamic relative imports only against the exact private server files, preserve valid static
  import cycles, and reject missing files, invalid paths, import attributes, failed evaluation
  and unfinished top-level await. Use each server file's own source map for JavaScript stacks.
  Depends on: S-5, S-6, S-9, S-10, S-13-2-2-1, S-13-2-2-1-1. Evidence: tracked multi-chunk, dynamic import,
  static cycle, failure, global isolation and stack tests; `make check` passes.
- [o] S-14 Implement the development mode with notify 8.2.0: a file change rebuilds the bundles and
  replaces the pool. Depends on: S-9. Acceptance: construction builds the initial server and watches
  the absolute application root; a source change rebuilds all bundles and replaces the server and pool
  together; a failed rebuild reports its cause and does not serve an outdated build; a later valid change
  restores service. Evidence: a file event changes rendered and public output; failure and recovery cases pass.
- [ ] S-15 Implement the Vue, Svelte and vanilla adapters and `features.json`. Depends on: S-15-1,
  S-15-2, S-15-3, S-15-4. Evidence: every declared feature has an executable case.
- [o] S-15-1 Implement the vanilla adapter. Depends on: S-7. Acceptance: absolute server and client
  application paths produce bundle entries; SSR returns HTML and output state, CSR preserves input
  state without using the pool, both use one client URL, and a static CSR shell accepts only empty
  props and null state. Invalid paths, URLs, HTML and input return errors. The browser test verifies
  that hydration retains the server DOM node and CSR renders. Evidence: build, render and browser
  cases; `make check`.
- [o] S-15-2 Implement the Vue adapter. Depends on: S-7, S-13-1.
  Acceptance: one application component produces server and client entries; SSR awaits Vue
  `renderToString`, returns HTML and output state, and the browser hydrates the existing DOM node.
  CSR and static shells follow S-15-1. Evidence: build, render and browser cases; `make check`.
- [ ] S-15-3 Compile Svelte application source for both server and client inside the Rust build
  process and implement the Svelte adapter. Depends on: S-7, S-15-3-1. Acceptance: source compilation needs
  no Node process; SSR and CSR share the client build; the browser retains the server DOM node
  after hydration; output state and static shells follow S-15-1. Compiler, build and render errors
  fail explicitly. Evidence: source build, render and browser cases; `make check`.
- [o] S-15-3-1 Require a head string in every runtime render result and carry its bytes in
  `RenderResult`. Depends on: S-2, S-6. Acceptance: missing, non-string and invalid Unicode head
  values fail; React, Vue and vanilla entries return an empty head, and their adapters reject
  nonempty head output until they define document placement. Evidence: runtime result and adapter
  rejection cases; `make check`.
- [ ] S-15-4 Record supported features in `features.json`. Depends on: S-7, S-15-1, S-15-2,
  S-15-3. Acceptance: each true feature names its executable evidence; no feature is inferred
  solely from source inspection. Evidence: a checker runs every referenced case and `make check`.
