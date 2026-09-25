[Korean](checklist.ko.md)

# Checklist

States: `[ ]` waiting, `[~]` in progress, `[o]` complete, `[!]` temporarily bypassed. Use `[!]`
only when an unfinished item must be deliberately bypassed because work otherwise cannot advance
to the next checklist item; do not defer a difficult item while work remains possible. A bypass is
not completion. Record its cause and retry condition in the item, resume without permission when
the condition is met, and audit only `[!]` items and those records without repeating unrelated
full test suites. [AGENTS.md](../AGENTS.md) governs the
procedure. Each item names its dependencies and its completion evidence.

## Requirements

- One Rust process bundles and renders; no Node process runs at build time or at render time.
  `npm ci` installs packages before a build.
- A render call crosses no foreign function boundary: the caller is Rust and calls the library
  directly.
- The bundle is loaded and compiled once per V8 isolate, not sent or compared on every call.
- Props enter the isolate once per call as a V8 value; the result leaves as bytes without a second
  JSON round trip.
- A context reset restores the global state from a snapshot instead of rebuilding it.
- ssr provides: SSR or CSR selected per render with one client bundle, the render state
  input and output for hydration, static CSR shells, publication of the public build files into a
  directory, serving of the build files, a bounded pool with a queue, a render timeout that
  terminates a running script, and operating system random values.

## Items

- [o] S-0 Initialize the local Git repository with the author `min-median-max`, `AGENTS.md`, this
  checklist, the changelog and `.gitignore`. Depends on: none. Evidence: the first commit;
  `git remote -v` prints nothing.
- [o] S-0-1 Add the requirements of ssr and the build and engine checks before
  implementation, and order the items by these requirements. Depends on: S-0. Evidence: this change is
  committed.
- [o] S-0-2 Define the temporary bypass state in both language versions of the procedure and
  checklist, and include it in the checklist state check. Depends on: S-0-1. Evidence: the document
  check validates both language versions and this change is committed.
- [o] S-0-2-1 Limit temporary bypass to an unfinished item that prevents progress to the next
  checklist item; prohibit deferring a difficult item while work remains possible. Depends on:
  S-0-2. Evidence: the document check includes this sub-item and this change is committed.
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
- [ ] S-6 Implement `ssr-runtime`: a pool of isolates with one isolate per worker thread and a
  bounded queue, the bundle compiled once per isolate, own operations for `console` and
  `crypto.getRandomValues` from the operating system, no fetch, I/O timers, files or network, a
  timeout that terminates a running script, and global state reset between requests. Depends on:
  S-2, S-4. Evidence: a global change of request A is absent in request B; a script that does not end
  is terminated at the timeout; a full queue returns an error; the Web API cases pass.
- [ ] S-7 Implement the React adapter: SSR renders HTML with the render state
  output; CSR returns the static shell of the same client bundle; the client hydrates the SSR HTML
  or renders the CSR shell. Depends on: S-5, S-6. Evidence: SSR, CSR and hydration cases in a browser
  test.
- [ ] S-8 Implement publication and serving: the public build files are written into a directory
  atomically; an existing file with equal content stays; an existing file with other content is an
  error; the build files are served with their content types. Depends on: S-5. Evidence: the three
  publication cases and a serving case.
- [ ] S-9 Implement `ssr-server`: `POST /_render`, tracing metrics for render time, pool wait and
  heap, and stack traces mapped through source maps with sourcemap 9.3.2. Depends on: S-7, S-8.
  Evidence: the HTTP cases and a mapped stack trace test.
- [ ] S-10 Implement snapshots keyed by the server bundle hash, the deno_core version and the library
  version; a context reset restores from the snapshot; a key mismatch creates a new snapshot.
  Depends on: S-6. Evidence: a mismatch test and a reset test.
- [ ] S-11 Implement `make bench`: renders per second and p50/p99 latency for a fixed page with 1, 4
  and 16 concurrent calls, and time and memory of a context reset; record the limits in
  `docs/benchmarks.md`. Depends on: S-9, S-10. Evidence: the benchmark output and the recorded limits.
- [ ] S-12 Pin the V8 archive by path and SHA-256 as recorded by S-4; a failed download is an error.
  Depends on: S-4. Evidence: a build without network succeeds.
- [ ] S-13 Stream React output with `renderToReadableStream`: status and headers are fixed when the
  shell is ready; an error before the shell returns 500 with an error page; an error after the shell
  renders the error boundary in the stream and is logged; every inline script and style includes the
  nonce of the call. Depends on: S-7. Evidence: the three cases pass.
- [ ] S-14 Implement the development mode with notify 8.2.0: a file change rebuilds the bundles and
  replaces the pool. Depends on: S-9. Evidence: the output changes after a file change.
- [ ] S-15 Implement the Vue, Svelte and vanilla adapters and `features.json`. Depends on: S-7.
  Evidence: every feature has evidence.
