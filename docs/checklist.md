# Checklist

## Items

- [o] S-0 Initialize the local Git repository with the author `min-median-max`, `AGENTS.md`, this
  checklist, the changelog and `.gitignore`. Depends on: none. Evidence: the first commit;
  `git remote -v` prints nothing.
- [o] S-0-1 Specify the render, build, and engine requirements of ssr and their
  verification items. Depends on: S-0. Evidence: the requirements in
  [Requirements](requirements.md) and the build and engine
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
- [o] S-1-4 Enforce test ownership across the workspace. For each declared shared behavior,
  run a base test in its owning crate and an actual use test in every consuming crate. Reject a
  missing, misplaced, empty, failed, or timed-out test and a declaration that only names a file.
  Depends on: S-1. Acceptance: the declarations cover the shared page and build behavior and every
  direct workspace consumer of each owner; each case names one integration test target and exact
  test function in its crate. Evidence: mutation cases fail for each invalid declaration, the
  declared tests execute in their own crates, and `make check` runs the ownership check.
- [o] S-2 Implement `ssr-core`: the render call `Page{render,title,language,props,state}` of
  `POST /_render` with the render mode `ssr` or `csr`, its result (HTML and the output state), and its
  errors. Cache headers are not part of this crate. Depends on:
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
- [o] S-5-1 Handle CSS imports in the React bundle explicitly and test their application build path.

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
- [o] S-6-3 Cancel a request when its stream closes or its configured duration expires, and
  acknowledge worker cleanup before returning its capacity. Priority: P0. Depends on: S-6,
  S-13-2. Acceptance: a stopped reader cannot block a native chunk send indefinitely; late
  cancellation cannot terminate the next request; pool health and close report an unresponsive
  worker. A worker or monitor thread unwind notifies `Pool::wait` without another render call.
  `PoolOptions` declares input, waiting-byte, output, chunk and heap limits, and
  `Cancellation` identifies one request. A large source chunk is split into bounded fragments
  without changing the output bytes. HTTP input exhaustion returns 413 and queue exhaustion
  returns 503. Evidence: tracked cancellation, timeout, backpressure, capacity, shutdown and
  byte-limit cases fail before correction and pass afterward; owner and consumer stream cases
  and `make check` pass.
- [o] S-7 Implement the React adapter: SSR renders HTML with the render state
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
- [o] S-8-1 Persist and read complete immutable build directories. Priority: required before a
  renderer starts from prepared build files. Depends on: S-5, S-8. Acceptance: `Build::write`
  atomically publishes all manifest, private and public files under a deterministic SHA-256
  directory; equal writes preserve that directory. `Build::read` rejects missing, changed, extra,
  duplicate, unsafe or symbolic-link files and invalid manifest fields. Failed publication leaves
  previous builds intact and reports cleanup failures. Evidence: tracked RED/GREEN persistence,
  integrity, concurrent publication and public-file selection cases, and `make check`.
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
- [o] S-9-1 Run development builds and render servers in separate Rust processes with explicit `ProcessOptions` for preparation, shutdown, restart count, event capacity and probe bytes. Priority: required for immutable render process snapshots. Depends on: S-9, S-14, S-8-1. Acceptance: the build command produces a verified immutable build directory; the render command serves that directory and completes an actual SSR request before receiving traffic; replacements preserve responses already using the previous process and stop and wait for that process after those responses finish. A failed build or preparation makes new requests fail explicitly. An unexpected render process exit is observed without polling and starts a replacement from the same completed build. Shutdown stops and waits for children and reports forced termination. Evidence: tracked process, readiness, failed rebuild, streaming drain, exit, restart and cleanup tests; `make check` passes.
- [o] S-9-1-1 Use a private Unix socket for supervised renderer requests. Priority: required for process readiness without service DNS. Depends on: S-9-1. The supervisor creates and owns the socket directory before spawning the child, passes the exact socket path to the command factory, rejects a different declared path and removes only its own directory after collecting process exit. Acceptance: tracked RED and GREEN cover an absolute private socket, request and response streaming, preparation, replacement, drain, cancellation, shutdown and socket cleanup. Readiness rejects a network address, a relative or different declared path, a declared path that is not a socket, a socket directory open to other users and a socket directory with another entry, which the supervisor keeps; a socket path longer than a Unix socket address is rejected before the directory is created. The restart cases of `crates/ssr-server/tests/development.rs` run a real child on the socket through the public `Development` API and require that every socket directory is removed after its process exit. Red: the path length case failed because `Socket::create` created the directory and returned no error; the non-socket and open directory cases failed because both returned `render socket requires a private directory`. The other new cases passed on their first run, and each failed against a mutation that was not committed: without the path comparison the network address, relative path and different path cases failed with `render socket path must be absolute and canonical` or a preparation timeout; with `remove_dir_all` the other entry case failed with `the supervisor removed a file it did not create`; with directory mode 0755 the absolute socket case failed; without the socket removal after process exit 7 development cases failed with `socket directory remains`, and without it after a failed preparation the preparation case failed; with a response body that is never dropped the cancellation case failed with `shutdown timed out; forced termination`. Green: the 19 unit cases of `ssr-server` and the `process` case pass, and 15 of the 16 `development` cases pass; the remaining case fails intermittently from the source watch, also on main, and is S-9-1-2. `cargo clippy -p ssr-server --all-targets -- -D warnings` and `cargo fmt --all -- --check` pass.
- [ ] S-9-1-2 Observe source changes written right after `Development::start` returns. Priority: required before the next full suite run. Depends on: S-9-1. Cause: a source change written right after `Development::start` returns can produce no change event, and the files written right before it can produce a second build, so the development cases that write a source change after start fail intermittently. Evidence: on main S-20-2, `changes_replace_render_and_public_files_and_fail_explicitly` failed in 1 of 3 runs with `change event timed out`; with the changes of S-9-1-1 it failed in 2 of 3 runs with `change event timed out` and once with a second build (`left: 2`, `right: 1` at `development.rs:16`), `source_file_replacements_remain_watched` failed with `change event timed out`, and `failed_rebuild_starts_the_previous_process_shutdown_duration` failed with `failed rebuild must start the configured shutdown duration` from its 3 s limit inside the test. Acceptance: a change of a source path written after `Development::start` returns produces a change event, and files written before it produce no further build; a deterministic RED case shows the missing event; the three cases are judged by events, without a time limit inside the test.
- [o] S-9-1-3 Rebuild when the source watch reports dropped events. Priority: required before the next full suite run. Depends on: S-9-1. Cause: when the file event service drops events, `notify` reports an event of kind `Other` with the `Rescan` flag, and `source_event` accepts only create, modify, remove and any events, so the supervisor discards the report and the dropped changes never reach a build. Acceptance: a `Rescan` event of a source path starts a rebuild; an excluded path still does not. The supervisor logs a warning for such a rebuild. Red: `dropped_event_reports_rebuild_unless_only_excluded` failed at `assertion failed: source_event(&rescan, &excluded)`. Green: the case passes with a `Rescan` event of the source root, a `Rescan` event without a path (the inotify queue overflow) and an excluded `Rescan` event that does not rebuild; the 21 unit cases of `ssr-server`, `cargo clippy -p ssr-server --all-targets -- -D warnings` and `cargo fmt --all -- --check` pass.
- [ ] S-9-1-4 Build once for the source events that arrive while a build runs. Priority: required before the next full suite run. Depends on: S-9-1-2. Cause: `notify` reports one file write as several events, and the supervisor starts a rebuild for each event in turn, so one write builds the application several times and the served build changes several times. Evidence: one `fs::write` of a new file produced `Create(File)`, `Modify(Metadata(Extended))` and `Modify(Data(Content))` events for the same path on the FSEvents backend. Acceptance: the events that arrive while a build runs start one rebuild after it, which reads the sources at its start; no time-based debounce is used. A deterministic RED case queues several events for one write and counts the builds; GREEN shows one rebuild.
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
- [o] S-13-3 Publish the request-nonce document rewriter as the reusable `ssr-nonce` crate and
  make the server use it instead of its private copy. The crate applies one request nonce to
  every `script` and `style` element of a streamed or complete document, preserves raw text and
  comments, and fails when an element already has a different nonce or the document is not
  UTF-8. Depends on: S-13-2-2. Evidence: the crate's split-tag, raw-text, comment, mismatched
  nonce and UTF-8 cases pass, the server delegates to the crate and its document boundary case
  passes, and `make check` passes.
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
- [o] S-13-2-2-1-2 Verify the official target-specific Linux V8 archive and matching generated
  binding in the development image. Depends on: S-13-2-2-1-3. Evidence required: archive
  and binding digest verification, native same-snapshot parallel rendering in that image, and a
  second image build without V8 source compilation. No archive download or source substitution.
- [o] S-13-2-2-1-3 Use the verified official local V8 archive and matching binding with one immutable
  snapshot key per renderer process. Priority: required before renderer integration. Dispose the
  public snapshot creator before restoring the same blob concurrently; retain one initialization
  and independent request state. Reject another key even after pools close. Build and render run
  in separate Rust processes. Depends on: S-13-2-2-1. Evidence required: tracked contract RED,
  native official-archive proof, unchanged isolation and performance limits, and make check.
  A tracked subprocess case must reject Svelte compiler entry while snapshot workers are active
  before an incompatible V8 isolate can be created. Safe completed builds retain their render use path.
  The shared process contract excludes concurrent compiler and snapshot initialization and verifies
  complete creator disposal after failed initialization. Test ownership maps each actual public
  behavior use to its consumer case and rejects missing behavior or consumer declarations.
  Svelte compilation uses the transform hook so compiler and process errors preserve their causes
  through the bundler; public build tests require the specific cause.
  Cargo case runners forward compilation output as it arrives without a total build deadline;
  nextest retains each case's deadline and missing or failed case output remains an error.
  This item replaces the same-process multi-snapshot and source-group requirements of S-10,
  S-12, S-13-2-2-1 and S-13-2-2-1-1; their recorded completed evidence remains unchanged.
  Evidence: make check exited zero with the official macOS AArch64 archive and binding: 54 Python
  cases, 17 owner and consumer cases, 145 workspace cases, 12 feature cases and 3 benchmark unit
  cases passed. The 2 independent make verify-build cases and existing make bench limits passed.
  The engine-free build verification command requires no V8 inputs, verified by a tracked regression case.
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
- [o] S-15 Implement the Vue, Svelte and vanilla adapters and `features.json`. Depends on: S-15-1,
  S-15-2, S-15-3, S-15-4, S-15-5, S-15-6. Evidence: every declared feature has an executable case.
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
- [o] S-15-3 Compile Svelte application source for both server and client inside the Rust build
  process and implement the Svelte adapter. Depends on: S-7, S-15-3-1, S-13-2-2-2. Acceptance: source compilation needs
  no Node process; the server entry exports `render` as an ESM function; SSR and CSR share the client build; the browser retains the server DOM node
  after hydration; output state and static shells follow S-15-1. Component styles are published as
  hashed CSS in the build manifest and Svelte head output appears in the document head. The HTTP
  server serves the Svelte build through the ESM pool, applies the request nonce to inline head
  and body scripts and styles, and serves the published component CSS. Compiler,
  build and render errors fail explicitly. Generated JavaScript, CSS and JavaScript source maps are
  used, while compiler warnings fail. CSS source maps are not published after CSS transformation.
  Evidence: source build, HTTP style, head and nonce output, render and browser cases; `make check`.
- [o] S-15-3-1 Require a head string in every runtime render result and carry its bytes in
  `RenderResult`. Depends on: S-2, S-6. Acceptance: missing, non-string and invalid Unicode head
  values fail; React, Vue and vanilla entries return an empty head, and their adapters reject
  nonempty head output until they define document placement. Evidence: runtime result and adapter
  rejection cases; `make check`.

- [o] S-15-4 Record supported features in `features.json`. Depends on: S-7, S-15-1, S-15-2,
  S-15-3. Acceptance: each true feature names an exact test package, binary and case; duplicate
  declarations, missing cases, failed commands and timeouts fail; no feature is inferred solely
  from source inspection. Evidence: a checker executes every referenced case with an exact match,
  tracked negative checker cases and `make check`.
- [o] S-15-5 Verify the React adapter with an empty nested repeated form. Priority: required
  for S-15 completion. Depends on: S-7, S-6, S-13. Acceptance: the tracked fixture
  `FormRowsApp.tsx` renders a repeated row group that holds a repeated child group; one server render
  names the parent row and its child row `rows.<key>.<field>` with the same parent key
  and distinct row keys of `row-` and eight hexadecimal digits; a second render from the same pool creates a new parent key. Evidence:
  two executable render cases and `make check`.
- [o] S-15-6 Select hydration from the explicit page render mode in the React, Vue, Svelte and vanilla
  client entries. Depends on: S-7, S-15-2, S-15-3, S-13-2-2-2. Acceptance: the document preserves SSR or CSR
  mode even when the server body is empty; the client invokes hydration for an empty SSR body and
  a new render for CSR; an absent or invalid mode fails. Evidence: executable Chrome cases for all
  four adapters and `make check`.
- [o] S-15-6-1 Isolate the generated files of the React entry build and browser hydration tests.
  Priority: required for reliable workspace checks. Depends on: S-15-6. Acceptance: concurrently
  prepared test entries never replace another test's server, framework or client source; both
  tests build and assert their own output when run alone or in parallel. Evidence: a tracked
  concurrent overwrite case fails before the correction and passes afterward, each affected test
  and their parallel pair pass, and `make check` passes.
- [o] S-15-7 Resolve every application package import from the configured application dependency
  directory so nested `node_modules` copies cannot instantiate duplicate module state. Depends on:
  S-7, S-15. Acceptance: a fixture with the same package under the application source directory
  and the configured dependency directory uses one package path in the server bundle; a render
  context provider and its consumer share the same module instance. Evidence: the tracked
  resolver fixture fails before the correction and passes after it, and the consuming SSR request
  returns a document without a missing-context error.

- [o] S-15-7-1 Resolve configured dependency exports through the standard JavaScript resolver. Priority: required for consuming declared dependency directories. Cause: the custom dependency resolver ignores package exports and rejects exported subpaths. Acceptance: tracked RED cases reproduce a scoped conditional export failure and require rejection of an unexported subpath. The standard resolver passes both cases and the existing single-directory case; the page and browser cases of a dependency directory pass. Depends on: S-15-7.

- [o] S-16 Declare each local checkout path once in the root manifest. Priority: required before any build after a checkout moves. Cause: the V8, CSS and JSON checkout paths are repeated in three manifests, two tools and the engine Compose template, so a moved checkout leaves stale copies that fail only at the next build. Acceptance: every absolute path dependency in the root, build-probe and engine manifests names an existing checkout and matches the root manifest for the same crate; the engine Compose preparation reads its mount paths from the root manifest; the engine Compose template and mount check contain no host path and the mount check reads the mount paths from its environment. Evidence: the tracked case fails on the absent V8, CSS and JSON checkout paths and on the host paths of the engine Compose template before the correction; all six local path cases pass after it; locked offline Cargo metadata resolves; `make check` passes. Depends on: S-4, S-5, S-12.
- [o] S-17 Release the scheduler of a finished React stream render. Priority: required before a renderer serves more than a few hundred documents. Cause: each stream render creates a context whose slot holds the scheduler, and the scheduler holds handles to the callbacks that the render left queued, so the context of every finished render stays reachable and the isolate runs out of heap after some hundred renders. Acceptance: after each stream render, on success and on failure, the scheduler drops its queued callbacks and the context drops the scheduler; 600 stream renders on one isolate keep the heap within 16 MiB of its early size; the tracked heap case fails before and passes after the change. Depends on: none. Evidence: the heap case ran out of V8 heap within 600 renders before the change and passes after it.
- [o] S-18 Make the test runs follow the test rules: only the owning Red and Green tests run during development, the full suite runs once when every active checklist item is complete, and every test case is judged by events and its own timeout instead of host speed. Priority: required before the next full suite run. Depends on: none. Acceptance: S-18-1 to S-18-6 and their sub-items are complete. The full suite follows the procedure of `AGENTS.md`: `make check` runs once, after every active checklist item is complete, `var/full-run.json` records its result and step times, and a failure becomes a new checklist item. The acceptance required a passing `make check` while S-18 was in progress, which `tools/full_run.py` (S-18-6) refuses, so the full suite run is not evidence of this item. Evidence: S-18-1, S-18-2, S-18-2-1, S-18-3, S-18-3-1, S-18-4, S-18-4-1, S-18-5, S-18-5-1 and S-18-6 are complete.
- [o] S-18-1 State when the owning tests and the full suite run. Priority: required before the next commit. Cause: the procedure requires `make check` before each commit and the full suite whenever an item is marked complete, so every fix runs the full suite. Acceptance: `AGENTS.md` and `AGENTS.ko.md` require the owning Red and Green tests and `tools/check.py` before each commit and the full suite once, when every active checklist item is complete. Red and Green: no check reads these sentences, so the change is recorded as a document change. Depends on: none.
- [o] S-18-2 Judge the queued cancellation and timeout cases by events. Priority: required before the next full suite run. Cause: the cases spin with `yield_now` until the pool reports one waiting call and fail after 200 ms or 1 s, and the timeout case also fails after 2.5 s, so a slow host fails correct code. Acceptance: a test-only queue entry signal on a crossbeam channel reports that a call entered the queue; the cases wait for that signal, judge the result by the error kind and have no time limit other than the 30 s nextest limit. The cancellation case uses a 10 s request timeout so that only the cancellation can end its queue wait. Red: with a test-only delay of 300 ms before the cancellation call enters the queue and 1100 ms before the timeout call enters it, both cases fail with `call did not enter queue`. Green: with the same delays both cases pass. Depends on: none.
- [o] S-18-2-1 Prove that one deadline covers the queue wait and the execution. Priority: required before the next full suite run. Cause: S-18-2 removed the 2.5 s bound of the timeout case, so the case no longer distinguishes one request deadline from separate deadlines for the queue wait and the execution. Acceptance: the case uses a queue wait D of 2 s, an execution E of 2 s and a request timeout T of 3 s, so D < T, E < T and D + E > T; the same execution on a free worker completes, and the queued call ends in `Timeout`; the result alone decides the case, with no elapsed time bound. Red: with the deadline extended by the queue wait, which gives the execution its own budget, the queued call succeeds and the case fails on `matches!(waiting.join().unwrap(), Err(crate::Error::Timeout))`. Green: with the single request deadline the same case passes in 5 s. Depends on: S-18-2.
- [o] S-18-3 Judge the render benchmark limits by CPU time. Priority: required before the next benchmark run. Cause: the benchmark judges p99 latency, renders per second and context reset p99 by wall-clock time, so other work on the host fails a correct runtime. Acceptance: each call measures process CPU time with `clock_gettime(CLOCK_PROCESS_CPUTIME_ID)` through the pinned `libc` crate, because rendering runs on pool worker threads and the CPU time of the calling thread does not contain it; CPU renders per second, CPU p99 and the peak heap change are judged, and wall-clock throughput, wall-clock p50 and p99 and context reset times are only reported; `docs/benchmarks.md` records the definitions, a measured run and the limits; `cargo deny check` passes. Red: the `check_limits` case with a wall-clock sample ten times slower than the limits and a CPU sample within them fails with `concurrency=1 exceeded recorded benchmark limits`. Green: the same sample passes, CPU samples that exceed the throughput, p99 or heap limit still fail, and `make bench` passes on a host with a load average of about 150, where the wall-clock p99 of 5.2 ms at concurrency 1 exceeds the former 2 ms limit. Depends on: none.
- [o] S-18-3-1 Measure the render CPU time in the worker thread. Priority: required before the next benchmark run. Cause: the process CPU time of a call at concurrency 4 and 16 contains the CPU time of the other calls that run at the same time, so the CPU limits of those scenarios do not measure one render. Acceptance: with the `bench` feature of `ssr-runtime`, the worker measures `CLOCK_THREAD_CPUTIME_ID` through the pinned `rustix` crate from the start of the context reset to the serialized result and returns it as `RenderMetrics::render_cpu`; the feature is used only by the benchmark, `make check` builds and lints the benchmark with it, and the library keeps `forbid(unsafe_code)`; the benchmark judges CPU throughput and CPU p99 from those worker measurements with limits of 1,000 renders/s and 4 ms at every concurrency; `cargo deny check` passes. Red: the `cpu_time_of_a_call_excludes_concurrent_calls` case, which requires the CPU p50 at concurrency 16 to stay below four times the CPU p50 at concurrency 1, fails with the process CPU time: `CPU p50 alone=341µs concurrent=3.657ms`. Green: with the worker measurement the same case passes (`alone=512.834µs concurrent=316.75µs`), and `make bench` passes with a CPU p99 of 0.70 to 1.04 ms under a host load average of about 300. Depends on: S-18-3.
- [o] S-18-4 Decide the browser cases by events and bound them only for hangs. Priority: required before the next full suite run. Cause: the browser cases wait for the browser process with a 25 s limit inside the test and run under the global 30 s nextest limit, while they take 13 to 27 s on an idle host; under host load the browser close alone takes 11 to 30 s, so the cases fail although every browser check passes. Acceptance: each adapter test target runs its browser script through a `browser` test module that prints every script line with its elapsed time when it arrives and ends when the browser process exits; the in-test HTTP handlers end when the browser closes its connections instead of after a 3 s read timeout; the scripts wait for page load, DOM markers and page errors, print `RUN` and `DONE` for the launch, each checked path and the browser close, and take their step limits from `browser-steps.mjs` (60 s for a step and 300 s for the launch, about one hundred times their normal durations); a nextest override for `test(/^browser_/)` reports a running case every 60 s and stops it after 30 minutes. Red: under a host load average of about 300, the React recovery, React, Vue and vanilla browser cases fail with `browser timed out` after every browser check printed `PASS`. Green: under a host load average of about 260 to 340, the same four cases pass in 24 to 48 s, and the step lines show the browser close taking 11 to 30 s. Depends on: none.
- [o] S-18-4-1 Give the browser launch and close no time limit. Priority: required before the next full suite run. Cause: S-18-4 gave the browser launch a 300 s limit (`LAUNCH_TIMEOUT` in `browser-steps.mjs`), but a browser launch is a long operation, which gets step logs instead of a time limit; a launch slower than the limit fails a correct browser case by the clock and not by its result. The browser close has no limit. Acceptance: the five browser scripts launch the browser with `timeout: 0` and print `RUN launch` and `DONE launch` with the elapsed time; `browser-steps.mjs` keeps only `STEP_TIMEOUT` (60 s) for page steps; `tools/test_browser_launch.py` runs each script with a browser that starts 3 s late, with a 1 s `LAUNCH_TIMEOUT` injected through a `node:test` module mock, and requires `DONE launch` after at least 3 s, the first page step, and `RUN close` and `DONE close`. Red: with the 1 s limit the five scripts fail with `browserType.launch: Timeout 1000ms exceeded` and print no `DONE launch`. Green: the five cases pass in 37 s; each script reports the launch after about 3.8 s, starts its first page step and reports the close. Depends on: S-18-4.
- [o] S-18-5 Build the ownership cases once. Priority: required before the next full suite run. Cause: `tools/test_ownership.py` runs each case with `cargo nextest run -p <crate>`, which resolves features for that package alone, so the shared crates are compiled again for each feature set with one Cargo job and the later workspace test suite compiles them once more; the step took 597 s in `make check`, 400 s of it in one case. Acceptance: the checker builds every workspace test once with `cargo nextest run --workspace --no-run`, reporting the build as a separate step with its elapsed time and forwarding Cargo progress without a total duration limit, and then runs all declared cases in one `cargo nextest run --workspace` with a filter that names each case; each case keeps its own nextest timeout and is reported as passed or failed. Red: the `test_cases_run_once_on_one_workspace_build` case fails because the checker calls `cargo nextest run -p ssr-core --test page` without `--workspace`. Green: the same case passes. Measured after a change to `ssr-core` with `CARGO_BUILD_JOBS=1` under a host load average of 150 to 360: before, the step took 434 s with 23 crate compilations and the slowest cases took 121 s and 117 s; after, it took 268 s, of which the build took 243 s with 8 crate compilations and all 17 cases took 22.9 s, the slowest 20.6 s. Depends on: none.
- [o] S-18-5-1 Give the workspace metadata read no time limit. Priority: required before the next full suite run. Cause: `tools/test_ownership.py` runs `cargo metadata` with a 30 s limit; Cargo can resolve the index there, so it is a long operation, which gets step logs instead of a time limit, and a slow read fails the check by the clock. Acceptance: the checker prints `RUN cargo metadata ...` and `PASS cargo metadata <s>s` or `FAIL cargo metadata exit <code> <s>s`, passes Cargo standard error through as it arrives, applies no time limit and raises `workspace metadata failed with exit <code>` on a nonzero exit. Red: with a `cargo` that is silent for 2 s before running Cargo and the limit cut to 1 s, `test_cargo_metadata_runs_past_any_limit_to_its_exit` fails with `TimeoutExpired`, and with a `cargo` that exits 101, `test_cargo_metadata_failure_names_its_exit_code` fails because the error does not name the exit code. Green: both cases pass, the first reporting `PASS cargo metadata 2.x s` after the delay; the 27 cases of `tools/test_test_ownership.py` pass in 5.4 s. Depends on: S-18-5.
- [o] S-18-6 Refuse the full suite before any step unless it may run. Priority: required before the next full suite run. Cause: `make check` started its steps in any state of the checkout, with items in progress, with uncommitted changes and again on a tree whose full suite had run, so the rule of S-18-1 held only when the person who ran it applied it; a `make` without a target ran `check-steps` directly, without the checkout lock. Acceptance: `tools/full_run.py` decides before any step of `make check` and `make rerun-failed` and prints the decision with its reasons; it refuses with exit status 2 while an item of `docs/checklist.md`, sub-items included, is in progress, naming each ID and title, while tracked files have uncommitted changes, and, for `make check`, when `var/full-run.json` records a full run of the same tree (`git rev-parse HEAD^{tree}`), naming that run; the record holds the tree, the commit, the result, the targets that did not pass and the times of every step, and is written before the first step and after each step starts and ends, so a killed run stays `incomplete`; `make rerun-failed` runs the setup steps and only the targets of the record of the current tree that did not pass, and is refused without such a record; the steps of the former `check-steps` are the make targets of `CHECK_SETUP` and `CHECK_TARGETS`; a `make` without a target runs `make check`. Red: with `tools/full_run.py` present and the former Makefile, `test_make_check_starts_with_the_guard` failed because the `check` recipe ran `$(MAKE) check-steps` directly, so with a checklist holding items in progress the steps would start; the default goal of a `make` without a target was `check-steps`. Green: the 11 cases of `tools/test_full_run.py` pass on a temporary checkout with stub make targets: an active item and an active nested sub-item are refused and listed while no step runs, an uncommitted Makefile change is refused, a second full run of the same tree is refused naming the first, a rerun without a record is refused, a rerun runs the setup step and only the failed target `b` and then has nothing to rerun, and a run killed with SIGKILL stays `incomplete` with its target `running`; the 38 cases of the full run, check lock, run directory, archive, feature and workspace tool tests pass; `make check` in this checkout is refused with exit status 2, listing S-9-1-1 and S-18 and the uncommitted files, and runs no step. Depends on: S-18-1.
- [o] S-19 Keep the resources of one verification run from other runs. Priority: required before the next engine verification. Cause: concurrent runs of different checkouts or of one checkout use the same container stack and the same fixture installs. Acceptance: S-19-1 and S-19-2 are complete. Depends on: none.
- [o] S-19-1 Give each checkout its own engine verification stack and hold it by a lock. Priority: required before the next engine verification. Cause: the engine verification used a fixed Compose project name, a fixed container name and the image `localhost/ssr-engine-verify:0.0.1`, so a verification of another checkout replaced the stack while a run used it. Acceptance: `tools/prepare_engine_compose.py` names the project `ssr-engine-<12 hex digits of the SHA-256 of the checkout path>`, the container `<project>-engine` and the image `localhost/ssr-engine-verify-<digits>:0.0.1`; `make verify-engine-linux-arm64` and `make verify-engine-down` run `tools/verify_engine_linux.py`, which holds the checkout lock `var/locks/engine-verification.lock` of `tools/holder_lock.py` for every step and prints each step with its result and elapsed time without a time limit; a held lock refuses with the holder's checkout, pid and process start time; a lock whose holder no longer runs is reported and kept until `python3 -m tools.holder_lock remove-stopped` removes it; only the holder releases the lock. The engine Compose case renders the file without writing the checkout's `var/engine-compose.yaml`. Red: `tools/test_engine_stack.py` failed to import `tools.verify_engine_linux`, the Makefile, Compose template and status check named the fixed project and `ssr-engine-verify:0.0.1`, and `tools/test_holder_lock.py` failed to import `tools.holder_lock`. Green: the 4 stack cases and the 9 lock cases pass, and the 6 local path cases pass with the rendered file; `python3 -m tools.prepare_engine_compose` writes the project `ssr-engine-98e175b66c0f` for a worktree. Depends on: none.
- [o] S-19-2 Hold the fixture installs of a checkout for the whole `make check`. Priority: required before the next full suite run. Cause: the build-probe fixture installs are one directory per checkout; a second concurrent `make check` in the same checkout reinstalled them while the tests of the first read them. Acceptance: `make check` runs `python3 -m tools.holder_lock run check -- $(MAKE) check-steps`, which holds `var/locks/check.lock` from the first installation to the last test, passes SIGINT, SIGTERM and SIGHUP to the steps, releases the lock after them on success and failure, and refuses a second run with the holder's record; the target-specific exports and the archive check apply to `check-steps`. A test run outside `make check` does not take the lock; the fixture is prepared only by `make check`. Red: `tools/test_check_lock.py` failed its 3 cases: the `check` recipe ran its steps directly and `holder_lock` had no `run`. Green: the 3 cases pass (the lock exists while the command runs, is gone after exit 0 and exit 4, and a held lock refuses the run without running the command), `tools/test_verify_archive.py` expects `check-steps bench: verify-archive`, and the 22 cases of the Makefile-reading tool tests pass. Depends on: S-19-1.
- [o] S-20 Allow a state marker in the checklist only as the state of an item. Priority: required before a tool reads the checklist states again. Cause: the checklist began with a legend of the four state markers and the texts of S-18-1 and S-18-6 named markers in inline code, while `tools/check.py` compared only the item states of the English and Korean checklists, so a tool that counted markers counted items in progress that do not exist. Acceptance: `AGENTS.md` and `AGENTS.ko.md` define the states; `docs/checklist.md` and `docs/checklist.ko.md` have no legend, and their texts name states in words; `tools/check.py` fails for every bracketed state marker of either file that is not the state of an item line, an item line being a line that starts with a hyphen, a space, the marker, a space and an item ID, and names the file, line and column and the reason, without exceptions or an allow-list. Red: the fixture of `tools/test_checklist_markers.py` holds markers in a legend, a table cell, inline code of an item, prose of a continuation line and a second item written inside an item line; the former `check_pairs_and_links` of `tools/check.py` returned no error for it, so `test_marker_outside_an_item_state_names_its_location` failed with an empty list against the 12 expected errors. Green: the 3 cases pass: the fixture fails with the 6 locations of each file, a checklist with markers only as item states passes, and the repository checklists pass; with the check and the former checklists, `python3 tools/check.py` exited 1 naming 14 markers (lines 5, 362 and 371 of the English file and 5, 304 and 313 of the Korean file); after the legend was removed and the texts were rewritten it passes. Depends on: none.
- [o] S-20-1 Treat the task list states of GitHub as state markers. Priority: required before a tool reads the checklist states again. Cause: S-20 checks only the four states of this repository, while a Markdown reader also takes an x or a capital X between brackets as the state of a task list item, so such a marker outside an item state passed `tools/check.py`. Acceptance: `tools/check.py` fails for a bracketed space, tilde, o, exclamation mark, x or capital X in `docs/checklist.md` and `docs/checklist.ko.md` that is not the state of an item line, naming the file, line and column; an item line keeps the four states that `AGENTS.md` defines, so a line that starts with the x form is not an item line. Red: with an item line in the x form and a capital X in inline code added to the fixture of `tools/test_checklist_markers.py`, `test_marker_outside_an_item_state_names_its_location` failed because the former check reported none of the 4 expected errors at lines 10 and 11. Green: the 3 cases pass and `python3 tools/check.py` passes on the repository checklists. Depends on: S-20.
- [o] S-20-2 Keep only items and headings in the checklist. Priority: required before a tool reads the checklist again. Cause: the checklist began with a link to its translation, a paragraph on the procedure and the Requirements section, and no check told such text from items, so a reader of the file could not take every list item as an item. Acceptance: the requirements are in `docs/requirements.md` and `docs/requirements.ko.md`, which S-0-1 links; `AGENTS.md` and `AGENTS.ko.md` keep the procedure that the paragraph named; `tools/check.py` fails for every line of `docs/checklist.md` and `docs/checklist.ko.md` that is not blank, a heading, an item line or a continuation line of an item, a continuation line being a line indented by two spaces that directly follows an item line or another continuation line, and names the file, line and column. Red: `test_text_outside_an_item_names_its_location` failed because the former check returned no error for a fixture with a translation link, a paragraph, a list item under a heading with its second line and an indented paragraph after a blank line. Green: the 4 cases pass, the fixture failing with the 5 locations of each file; with the check and the former checklists `python3 tools/check.py` failed for lines 1 to 21 of the English file and 1 to 17 of the Korean file; after the requirements moved to `docs/requirements.md` and the paragraph and the link were removed it passes. Depends on: S-20-1.
