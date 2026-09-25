[Korean](changelog.ko.md)

# Changelog

## 0.0.1

- Add the development procedures (`AGENTS.md`), the checklist and this changelog.
- Add the render, build and engine requirements of ssr and the build and engine checks to the checklist.
- Create the Rust 1.98.1 workspace with eight crates and the check and bench commands. The check
  command validates records, terminology, documents, dependencies and unit tests because these
  requirements apply to every crate.
- Exclude installed package documents from record and terminology checks because packages are
  dependencies outside the maintained source.
- Verify the engine dependency versions and V8 archive digests. Full builds link for macOS arm64,
  macOS x86_64 and Linux x86_64. The Linux AArch64 build remains incomplete because the available
  linker rejects a required Rust target argument.
- Continue the Linux AArch64 engine build verification under S-4 because an isolated target build
  is in progress.
