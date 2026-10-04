[Korean](changelog.ko.md)

# Changelog

## 0.0.1

- Build the workspace tests once before the ownership cases and run every case
  in one nextest run on that build.

- Measure the render CPU time in the worker thread for the render benchmark,
  so concurrent calls no longer add to the CPU time of a call.

- Prove that one request deadline covers the queue wait and the execution with a
  queue wait and an execution that each fit in the timeout but not together.

- Decide the browser cases by page events and process exit, print each browser
  step with its elapsed time, and bound a browser case only to detect a hang.

- Judge the render benchmark by process CPU time: CPU renders per second and
  CPU p99 have limits, and wall-clock values and context reset times are
  reported only.

- Judge the queued cancellation and timeout cases by a test-only queue entry
  signal and the returned error kind instead of wall-clock limits.

- State that only the owning Red and Green tests and `tools/check.py` run
  before a commit, and that the full suite runs once when every active
  checklist item is complete.

- Release the scheduler of a finished React stream render, so the context of
  every finished render can be collected and repeated renders keep the heap
  bounded.

- Declare each local V8, CSS and JSON checkout path once in the root manifest.
  The engine Compose preparation and mount check read those paths, and a
  tracked case rejects an absent or differing path in any manifest.

- Resolve configured JavaScript imports with the standard resolver so scoped
  conditional exports select their declared entry and unexported subpaths fail.
  Preserve the single dependency directory and reject nested dependency replacements.

- Resolve every application package import from one configured dependency directory: bare
  package specifiers resolve only there, so a nested `node_modules` copy of the same package
  cannot create a second module instance beside the render context provider and consumer.

- Verify the official Linux V8 archive and binding inside the engine verification image: digest
  checks, read-only source mounts including ordered-json, and native same-snapshot parallel
  rendering with no V8 source compilation.

- Cancel closed and expired streams, bound native chunk transmission, and acknowledge cleanup before
  worker reuse. Require explicit pool byte and heap limits. Report unavailable workers through a
  closure event and return errors from thread cleanup.

- Use one immutable application snapshot per renderer process with the official local V8 archive
  and matching binding. Public snapshot creation completes before concurrent workers restore the
  same blob. Different bundle keys require separate renderer processes because one process retains
  one initialized application. Verification rejects missing inputs, changed hashes and mismatched
  features. Native parallel restoration, React streams, separate application processes and existing
  performance limits are checked with the official archive.
- Reject Svelte compiler entry during snapshot initialization and after a successful snapshot.
  A shared process contract protects V8 entry through isolate disposal; completed compilation may
  precede rendering, and ordinary Rust bundling remains available. Svelte transformation preserves
  compiler error causes. Test ownership requires actual behavior use and its declared consumer test.

- Run build and render commands in separate processes, validate completed build directories and
  actual SSR preparation before serving requests, retain active response processes during replacement,
  and restart an exited render process from its completed build. Report rebuild and shutdown failures,
  collect child exit statuses, and watch declared source inputs with explicit output exclusions.

- Add `Build::write` and `Build::read` for complete build directories named by the manifest
  SHA-256. Publish private and public files with one directory rename, preserve equal writes and
  previous builds, and reject changed manifests, missing or extra files, invalid paths, symbolic
  links and incorrect file digests. Report publication and cleanup failures together.

- Require a target-specific Linux V8 archive with independent isolate-group support for development image rendering.

- Resolve application packages from the configured dependency directory so nested package copies do not create separate module instances.

- Complete the Vue, Svelte and vanilla adapters. `features.json` names an exact executable case for
  every supported feature, and `make check` runs every declared case.
- Record supported adapter rendering, public asset, source map and React form capabilities in
  `features.json`. Each declared capability names one exact executable case. The feature checker
  runs every case, rejects missing evidence or duplicate feature declarations and fails on a missing test, nonzero
  command exit or timeout because a declaration or source inspection cannot establish support.
- Enforce test ownership for shared page and build behavior. Each owner and every direct
  consuming crate declares a specific integration test function, and `make check` executes
  each case. Missing, misplaced, empty, failed, ignored and timed-out tests fail because a
  file name alone does not prove behavior in the crate that uses it.
- Verify React streaming across HTTP shell status, late body errors, request nonces, timer
  isolation, render metrics, and browser recovery after a Suspense error.
- Verify that a React stream reader rejection after a body chunk returns that chunk and an
  explicit body error while the initial HTTP status and headers remain fixed.
- Correct the React adapter and runtime documents to name the framework and application entries,
  `Pool::new_react`, `Pool::render_stream`, and `ReactAdapter::stream_parts`. A record check rejects
  the obsolete React pool call because it describes a different render path.
- Stream React SSR documents after the shell is ready. Restore the framework and application in
  one request context, keep scheduling outside the application global, and preserve React's
  Suspense fallback and client recovery instructions. Generate a distinct operating-system nonce
  per request, apply it to inline scripts and styles across output chunks, and record React errors
  with that nonce. Reject React server chunks because its application bundle is an IIFE.
- Verify the React bundle execution boundary in an executable V8 fixture. A top-level React
  class and context use the same React instance while framework scheduling remains in a
  function argument and the application global has no timer.
- Remove procedure-only S-0-2-1 from the checklist and keep the priority and test placement
  rules in AGENTS.md. Checklist items require a repository artifact and verifiable completion
  evidence.
- Add Svelte source compilation inside the Rust build process and the Svelte adapter. The build
  publishes component CSS with a content-hashed URL; the adapter places server head output in the
  document. The HTTP server serves the ESM build and component CSS, applies a request nonce to
  inline head and body content, and maps JavaScript failures through the build source maps.
  Browser cases verify hydration, client events, head output, component style, CSR and static
  shells because generated bundles alone do not establish browser behavior.
- Select React, Vue, Svelte and vanilla client hydration from the render mode recorded in each document.
  An empty SSR body still selects hydration, while CSR selects a new render. Missing or invalid
  modes fail because body content does not identify the requested render operation.
- Verify a React form with repeated rows of the tracked fixture `FormRowsApp.tsx` with two
  server renders. The case checks nested `rows.<key>.<field>` names, row keys of `row-` and eight
  hexadecimal digits and a new key on the next render because each
  request must create independent row identities.

- Require a `head` string in every runtime render result and preserve its bytes. Missing or
  invalid values fail because render output must remain complete. React, Vue and vanilla return
  an empty head and reject nonempty head output without a document placement rule.

- Add the Vue adapter with Promise-based server rendering and browser hydration. The browser
  verifies the original server DOM node and an attached event handler because a generated bundle
  does not establish hydration behavior.

- Add the vanilla adapter with server and client entries, SSR output state, CSR documents and
  static shells. A browser verifies server DOM identity and client event handling because generated
  source alone cannot establish hydration behavior.

- Add the development procedures (`AGENTS.md`), the checklist and this changelog.
- Synchronize the combined pool wait and execution timeout case on queue admission because a
  thread start signal does not establish which render owns the worker.
- Add the render, build and engine requirements of ssr and the build and engine checks to the checklist.
- Create the Rust 1.98.1 workspace with eight crates and the check and bench commands. The check
  command validates records, terminology, documents, dependencies and unit tests because these
  requirements apply to every crate.
- Exclude installed package documents from record and terminology checks because packages are
  dependencies outside the maintained source.
- Verify the engine dependency versions and V8 archive digests. Full builds link for macOS arm64,
  macOS x86_64, Linux x86_64 and Linux AArch64. The Linux AArch64 program executes in a native
  container.
- Build with a verified local V8 archive and Cargo network access disabled. Missing or changed
  archives and failed downloads return errors.
- Bind the engine verification Cargo cache and build output to ignored host directories. The
  source remains read only, and a replacement engine container reuses the linked program.
- Select the maintained compile-time identifier macro package in the local V8 source checkout.
  Rebuild and link the engine on four targets, mount that checkout read only in the test container,
  and retain the cached build across container replacement because the previous macro package has
  no maintained release.
- Allow the specified dependency licenses in cargo-deny because build dependencies use licenses
  beyond MIT. Unknown licenses remain errors.
- Allow BSL-1.0 for xxhash-rust 0.8.18 and Apache-2.0 with the LLVM exception for
  dragonbox_ecma 0.1.12 because the build depends on these exact crate versions.
- Verify React TSX server and client bundles, CSS imports and hashed asset output with Rust APIs.
  Package CSS resolution and URL replacement require explicit source and asset handling.
- Define the page JSON contract and render result in `ssr-core`. Ordered JSON preserves object
  order and number tokens; invalid fields and input return errors.
- Exclude unpublished local packages from the license check because the JSON package is consumed
  from a local source checkout. Registry dependencies remain subject to the license policy.
- Reject repeated decoded object keys in page JSON at every depth because retaining only the
  last value discards request input.
- Build React TSX server and client bundles, CSS and hashed assets in `ssr-build`. The build
  returns all files and an ordered-json manifest; the asset URL hook gives both bundles the public
  absolute URL because a chunk-relative URL does not identify the file from a page document.
- Execute server bundles in a bounded `ssr-runtime` worker pool. Each worker owns an isolate,
  resets global state for each request, terminates scripts at the timeout,
  and provides console output and operating system random values. Invalid results and unavailable
  Web APIs return explicit errors because output must not be omitted or changed silently.
- Initialize server globals in a V8 snapshot keyed by the bundle hash and pinned versions.
  Restore each request context from the snapshot because rebuilding server globals on each
  render repeats initialization. Changed keys create snapshots; unused snapshot bytes are released.
  Bundle initialization returns an error at the configured timeout.
- Create an independent V8 isolate group for each server snapshot and restore its worker isolates
  in that group. The release source build provides separate pointer cages and external code space so
  distinct application snapshots restore concurrently; unavailable group support fails.
- Verify a separate local V8 source archive and its generated binding by SHA-256 before routine
  checks and benchmarks link them.
  Direct source verification remains explicit because repeated GN generation can rebuild the archive.
- Evaluate private server entries and chunks as ECMAScript modules. Generated entries export
  `render` without a global property; relative imports resolve only to supplied server files.
  Restore module data per request context and map each file's JavaScript stack through its own
  source map. Missing files and unfinished evaluation fail because the render code is incomplete.
- Provide UTF-8 `TextEncoder` in every render context because the React server bundle requires it.
  Encoding and bounded `encodeInto` handle non-ASCII text and unpaired surrogates; invalid calls
  fail without adding file, network or timer operations.
- Create the React adapter entries and document renderer. SSR returns React HTML and output state;
  CSR returns an empty root with unchanged input state, and a static shell can serve multiple paths.
  Escaped JSON prevents a request from closing its script element. A browser verifies hydration,
  CSR rendering and shell reuse because bundle inspection cannot verify DOM behavior.
- Publish verified public build files into an absolute directory. Publication preserves equal
  files and rejects changed files because published URLs must keep their bytes. Serve exact public
  URLs with their content types and digests; server output remains private.
- Record private deterministic server source maps in the build manifest. The server verifies the
  map and server bundle, renders `POST /_render`, and returns explicit HTTP failures. It records
  render time, pool wait and V8 heap use through tracing. JavaScript failures report source-mapped
  stack locations because generated bundle positions do not identify application source lines.
- Add a development server that rebuilds bundles after file events and replaces its server and
  render pool together. Invalid changes make requests fail with the build cause until a valid
  change restores service because serving older output would hide the failed build.
- Measure a fixed SSR page with concurrent calls in `make bench`. The command reports throughput,
  call latency, context creation time and V8 heap change, and fails when a measured limit is exceeded.
- Complete V8 microtasks before reading a Promise render result. Fulfilled results use the normal
  result contract; rejection returns its message and stack, and a result with no scheduled
  completion fails. A separate-context React test verifies that an application component renders
  without receiving a framework timer because framework scheduling must remain private.
- Build a separate private React framework bundle and application server bundle with one shared
  React instance because independent copies reject application hooks. The build validates the
  framework input, records its digest and source map, and keeps it outside public files. A test
  renders a component using `useId` in a context without the framework timer.
- Give the React entry build and browser hydration tests separate directories for generated
  server, framework and client files. A concurrent test verifies that each reads its own files
  because a shared path can replace build inputs during parallel checks.
- Handle React CSS imports as explicit JavaScript-free modules and preserve data URLs during stylesheet bundling.
