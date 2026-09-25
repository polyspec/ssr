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
- Allow the specified dependency licenses in cargo-deny because build dependencies use licenses
  beyond MIT. Unknown licenses remain errors.
- Allow BSL-1.0 for xxhash-rust 0.8.18 and Apache-2.0 with the LLVM exception for
  dragonbox_ecma 0.1.12 because the build depends on these exact crate versions.
- Verify React TSX server and client bundles, CSS imports and hashed asset output with Rust APIs.
  Package CSS resolution and URL replacement require explicit source and asset handling.
