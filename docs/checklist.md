[Korean](checklist.ko.md)

# Checklist

States: `[ ]` waiting, `[~]` in progress, `[o]` complete. [AGENTS.md](../AGENTS.md) governs the
procedure. Each item names its dependencies and its completion evidence.

## Requirements

- One Rust process bundles and renders; no Node process runs at build time of the server or at
  render time. `npm ci` installs packages before a build.
- A render call crosses no foreign function boundary: the caller is Rust and calls the runtime
  directly.
- The bundle is loaded and compiled once per runtime, not sent or compared on every call.
- Props enter the runtime once per call as a V8 value; the result leaves as a stream of bytes.
- React renders with `renderToReadableStream`; a non-streaming call collects the same stream.
- A context reset restores the global state from a snapshot instead of rebuilding it.

## Items

- [o] S-0 Initialize the local Git repository with the author `min-median-max`, `AGENTS.md`, this
  checklist, the changelog and `.gitignore`. Depends on: none. Evidence: the first commit;
  `git remote -v` prints nothing.
- [ ] S-1 Create the Cargo workspace (`rust-toolchain.toml` with Rust 1.98.1, edition 2024) with the
  crates of [AGENTS.md](../AGENTS.md) and the Makefile targets `check` (rustfmt, clippy with
  `-D warnings`, `cargo deny check`, the record, terminology and document checks, the unit tests with
  cargo-nextest 0.9.146 and a timeout per test) and `bench`. Depends on: S-0. Evidence: `make check`
  exits with 0.
- [ ] S-2 Implement `ssr-core`: the render call `Page{render,title,language,props}` of
  `POST /_render`, its result, its errors and the cache header constants (`public, max-age=60,
  stale-while-revalidate=300`, `private, no-store`, `Vary: Accept-Language`). Depends on: S-1.
  Evidence: the JSON fixtures round-trip through ordered-json.
- [ ] S-3 Implement `ssr-build` with rolldown 1.2.11: server and client bundles from TSX and TS,
  CSS, content-hashed file names and the manifest. Depends on: S-2. Evidence: a sample application
  builds; the hashes are stable across two builds; the CSS file is emitted.
- [ ] S-4 Implement `ssr-runtime` with deno_core 0.412.0, deno_webidl 0.259.0 and deno_web
  0.290.0: a pool with one runtime per worker thread, the bundle compiled once per runtime, own
  operations for `console` and `crypto.getRandomValues`, and no fetch, I/O timers, files or network.
  Depends on: S-2. Evidence: a global change of request A is absent in request B; the Web API cases
  pass; a call with a new bundle hash replaces the runtime instead of recompiling in place.
- [ ] S-5 Implement the React adapter with `renderToReadableStream`: status and headers are fixed
  when the shell is ready; an error before the shell answers 500 with an error page; an error after
  the shell renders the error boundary in the stream and is logged; every inline script and style
  carries the nonce of the call. Depends on: S-3, S-4. Evidence: the three cases pass.
- [ ] S-6 Implement snapshots keyed by the server bundle hash, the deno_core version and the library
  version; a context reset restores from the snapshot; a key mismatch creates a new snapshot.
  Depends on: S-4. Evidence: a mismatch test and a reset test.
- [ ] S-7 Implement `ssr-server`: `POST /_render`, tracing metrics for render time, pool wait and
  heap, and stack traces mapped through source maps with sourcemap 9.3.2. Depends on: S-5.
  Evidence: the HTTP cases and a mapped stack trace test.
- [ ] S-8 Implement the development mode with notify 8.2.0: a file change rebuilds the bundles and
  replaces the pool. Depends on: S-7. Evidence: the output changes after a file change.
- [ ] S-9 Implement the Vue, Svelte and vanilla adapters and `features.json`. Depends on: S-5.
  Evidence: every feature has evidence.
- [ ] S-10 Pin the V8 archive by path and SHA-256; a failed download is an error. Depends on: S-4.
  Evidence: a build without network succeeds.
- [ ] S-11 Implement `make bench`: renders per second and p50/p99 latency for a fixed page with 1,
  4 and 16 concurrent calls, and time and memory of a context reset; record the limits in
  `docs/benchmarks.md`. Depends on: S-6, S-7. Evidence: the benchmark output and the recorded limits.
