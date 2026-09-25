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

`make verify-engine-linux-arm64` builds an ARM64 Linux container image from the
Rust 1.98.1 Bookworm image digest
`sha256:5b993f23fb69746405496e76f91e511d96c82b5b23304fd5d20b4a80a8b223ea`.
It downloads a missing archive and verifies its digest before mounting this
checkout read only at `/src`. It bind mounts the host's ignored
`var/engine-cargo` and `var/engine-target` directories at `/cargo` and `/target`
with write access, builds the program and executes it. `make` supplies absolute
host paths in the ignored `var/engine-compose.yaml` file so that later
`containerctl status` calls can read the same mount configuration. The host can inspect these directories, and
replacing the container preserves their contents. The image, container and
hostname use the permitted test installation name. The native GNU linker
retains the Rust target's link arguments.

## Build evidence

The macOS and Linux x86_64 builds ran on macOS arm64 with Rust 1.98.1 and Zig
0.16.0. The Linux AArch64 build ran in the ARM64 Linux container with Rust
1.98.1. These results distinguish a full target build and link from execution
on the target operating system.

| Target | Full build and link | Execution | Evidence |
| --- | --- | --- | --- |
| `aarch64-apple-darwin` | Pass | Pass on the host | Mach-O arm64 executable |
| `x86_64-apple-darwin` | Pass | Pass through Rosetta | Mach-O x86_64 executable |
| `x86_64-unknown-linux-gnu` | Pass with the Zig cross linker | Not run on Linux | ELF x86-64 executable |
| `aarch64-unknown-linux-gnu` | Pass in an ARM64 Linux container | Pass in the same container | ELF AArch64 executable; SHA-256 `3441cc6717b47e05969ae39844612bbc4ff213dd88ec7df14b89943bf65ef1bb` |

The native AArch64 Linux build completed with Rust 1.98.1 and the archive digest
listed above. Mount inspection confirmed `/src` as read only and `/cargo` and
`/target` as writable host mounts. `containerctl status` read the generated
Compose file without a project error. After replacing only the engine container,
the cached `deno_core 0.412.0` source and linked program remained visible on the
host. The repeated Cargo build finished in 0.51 seconds without compiling a
crate; the executable still had SHA-256
`3441cc6717b47e05969ae39844612bbc4ff213dd88ec7df14b89943bf65ef1bb`
and ran in the replacement container. No named volume is required. Linux x86_64 was fully linked
through a cross linker on macOS; execution on Linux x86_64 was outside this
build criterion.
