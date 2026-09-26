[Korean](changelog.ko.md)

# Changelog

## 0.0.1

- Add the development procedures (`AGENTS.md`), the checklist and this changelog.
- Add the requirements of the first consumer and the build and engine checks to the checklist.
- Create the Rust 1.98.1 workspace with eight crates and the check and bench commands. The check
  command validates records, terminology, documents, dependencies and unit tests because these
  requirements apply to every crate.
- Exclude installed package documents from record and terminology checks because packages are
  dependencies outside the maintained source.
- Verify the engine dependency versions and V8 archive digests. Full builds link for macOS arm64,
  macOS x86_64, Linux x86_64 and Linux AArch64. The Linux AArch64 program executes in a native
  container.
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
- Execute server bundles in a bounded `ssr-runtime` worker pool. Each worker compiles the bundle
  once, resets global state with a new context for each request, terminates scripts at the timeout,
  and provides console output and operating system random values. Invalid results and unavailable
  Web APIs return explicit errors because output must not be omitted or changed silently.
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
