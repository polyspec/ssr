[Korean](engine.ko.md)

# Engine verification

## Versions and archive acquisition

The verification program in `verification/engine` pins `deno_core =0.412.0`,
`deno_webidl =0.259.0`, and `deno_web =0.290.0`. Its committed lockfile resolves
`deno_v8 0.4.0` and `v8 150.4.0`. The program constructs a `JsRuntime` with the
`deno_webidl` and `deno_web` extensions so that the final link includes all three
required crates.

`deno_v8 0.4.0` selects `v8 150.4.0` through its default `v8` feature. The
[`v8` build script](https://docs.rs/crate/v8/150.4.0/source/build.rs) obtains a
prebuilt archive from
`https://github.com/denoland/rusty_v8/releases/download/v150.4.0/`.
The selected features include `simdutf`, so the archive name is
`librusty_v8_simdutf_release_<target>.a.gz`. The `RUSTY_V8_ARCHIVE` environment
variable selects an absolute local archive path. The build script accepts this
path without checking SHA-256; `tools/verify_engine.py` verifies the compressed
archive before invoking Cargo.

| Target | Compressed archive SHA-256 |
| --- | --- |
| `aarch64-apple-darwin` | `5aeffd8d5a0c1b79ac1d70af83d5b19099655fd9c645a794dc43f101f779838c` |
| `x86_64-apple-darwin` | `a750271fec6b211457ed0a5cf7d2eab1924b265621a82da86ab959d6ff0823e4` |
| `aarch64-unknown-linux-gnu` | `539e283815a396a5796f32858b42e517b858ebaaeaaad05d03290ee8c864a527` |
| `x86_64-unknown-linux-gnu` | `f48762ca10d1f1fc605a441c5ae430ec8ce1e9e80f14d78fbc42cb878c30b476` |

Run `python3 tools/verify_engine.py <target> --fetch` to download a missing
archive, verify its SHA-256, and perform a full `cargo build --locked --target`
of the verification program. Omitting `--fetch` requires an existing archive in
`var/v8`. A missing archive, changed digest, failed download, failed build or
timeout is an error.

## Build evidence

The build host was macOS arm64 with Rust 1.98.1 and Zig 0.16.0. These results
distinguish a full target build and link from execution on the target operating
system.

| Target | Full build and link | Execution | Evidence |
| --- | --- | --- | --- |
| `aarch64-apple-darwin` | Pass | Pass on the host | Mach-O arm64 executable |
| `x86_64-apple-darwin` | Pass | Pass through Rosetta | Mach-O x86_64 executable |
| `x86_64-unknown-linux-gnu` | Pass with the Zig cross linker | Not run on Linux | ELF x86-64 executable |
| `aarch64-unknown-linux-gnu` | Fail at final link | Not run on Linux | Zig 0.16.0 rejects `--fix-cortex-a53-843419` |

The AArch64 Linux Rust target supplies `--fix-cortex-a53-843419` to its linker.
The installed Zig linker rejects that argument. The installed LLVM linker
accepts it, but the LLVM Clang driver has no AArch64 GNU startup files or
libraries. S-4 remains in progress. Retry the full AArch64 Linux build with an
AArch64 GNU linker and libraries that accept the Rust target's link arguments,
or in an isolated AArch64 Linux build environment. Record the resulting linked
executable before completing S-4.
