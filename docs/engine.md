[Korean](engine.ko.md)

# Engine verification

## Versions and local inputs

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
path without checking SHA-256; `tools/verify_archive.py` verifies the compressed
archive and matching generated binding before invoking Cargo. It checks the exact V8 version
and feature set from offline locked Cargo metadata for the selected target. The `make check`,
`make bench` and engine verification commands reject source-build settings
and select only verified local inputs. Direct Cargo commands require the same input verification
and explicit archive and binding environment variables before execution.

The workspace and the engine verification use the `v8 150.4.0` crate of crates.io with the
official prebuilt archive. Its compile-time macro dependency `paste 1.0.15` has the maintenance
advisory RUSTSEC-2024-0436 and no patched release; `deny.toml` ignores that advisory with its
reason: `paste` is a compile-time proc-macro without runtime code, and the advisory reports no
vulnerability. It is reviewed again when an advisory reports a vulnerability in `paste` or when
`v8` replaces it.

| Target | Compressed archive SHA-256 |
| --- | --- |
| `aarch64-apple-darwin` | `5aeffd8d5a0c1b79ac1d70af83d5b19099655fd9c645a794dc43f101f779838c` |
| `x86_64-apple-darwin` | `a750271fec6b211457ed0a5cf7d2eab1924b265621a82da86ab959d6ff0823e4` |
| `aarch64-unknown-linux-gnu` | `539e283815a396a5796f32858b42e517b858ebaaeaaad05d03290ee8c864a527` |
| `x86_64-unknown-linux-gnu` | `f48762ca10d1f1fc605a441c5ae430ec8ce1e9e80f14d78fbc42cb878c30b476` |

`make verify-archive` checks the local archive and binding in `var/v8`. `make check` and
`make bench` execute it before linking. The binding name is
`src_binding_simdutf_release_<target>.rs`; the matching native features are exactly `simdutf`
and `use_custom_libcxx`. The `default` marker may be present because it selects `use_custom_libcxx`;
the native feature set is identical without that marker. Missing files, changed hashes, symbolic or relative paths and mismatched
Cargo features fail. `--archive`, `--binding` and `--metadata-root` may be supplied explicitly.
`--files-only` verifies staged files before any Cargo configuration is written; a normal build also
verifies the resulting dependency graph.

| Targets | Generated binding SHA-256 |
| --- | --- |
| macOS AArch64 and x86_64 | `ca5adf0cf89c9a70ad460ae73648b2fe89b74aa113b3cb7f757b6a02b758394f` |
| Linux AArch64 and x86_64 | `7727826ae479bdb645e807239fb12d1f8e2e23de7a6cf16f5ee592690d1d8506` |

`python3 tools/verify_engine.py <target>` requires the existing verified inputs and builds with
Cargo network access disabled. It records the full Cargo command and compiler activity without
imposing a total build duration limit. Execution on the native target has its own test time limit.

Linux is verified on the Linux runners of GitHub CI ([workspace](workspace.md)). On
`ubuntu-24.04-arm` and `ubuntu-24.04`, `make ci-setup` fetches the official archive and binding of
the runner's target and verifies their SHA-256 (`tools/fetch_v8.py`); the jobs `lint` and `test`
of `.github/workflows/ci.yml` then run Clippy on every target of the workspace (`check-clippy`),
build the `development_process` and `socket_process` examples (`check-examples`) and run every
test of the workspace (`ci-nextest`), which links `v8 150.4.0` and executes the engine on AArch64
and x86_64 Linux.

## Renderer snapshots

One renderer process selects one immutable bundle key. The public snapshot creator initializes
its context once, serializes it and is completely disposed before worker isolates restore that
same blob. A different key is rejected even after all pools close. Different bundles use separate
processes. Request contexts remain independent and do not evaluate either bundle again.

The compiler uses the default V8 snapshot, so its isolate creation and disposal are coordinated
with renderer snapshot creation through `polyspec_ssr_core::process`. A compiler cannot enter during
snapshot initialization or after its success. The snapshot creator is consumed on initialization
failure as well as success. The pinned V8 implementation removes shared read-only artifacts when
its last isolate is disposed; an executable test verifies failed initialization, subsequent Svelte
compilation and successful rendering in that order. A release build that accepts an incompatible
entry does not establish safe shared heap ownership, so the entry contract is checked before V8.

The official archive native tests on macOS AArch64 verify rejection of a changed process key and
80 renders across four parallel workers with identical initialized random data and no request
mutation. The React stream test verifies actual `renderToReadableStream` output for repeated
request contexts. The multi-process test renders three application bundles and one module
concurrently in separate processes. These tests are separate from cross-target build evidence.

## Build evidence

The macOS and Linux x86_64 builds ran on macOS arm64 with Rust 1.98.1 and Zig
0.16.0. The Linux AArch64 build ran in the ARM64 Linux container with Rust
1.98.1. These results distinguish a full target build and link from execution
on the target operating system.

| Target | Full build and link | Execution | Evidence |
| --- | --- | --- | --- |
| `aarch64-apple-darwin` | Pass | Pass on the host | Mach-O arm64; SHA-256 `39cd07f5f6a87f16cda29737753b8936244435ff391e5d8a3faff4dc0bdc3ad3` |
| `x86_64-apple-darwin` | Pass | Pass through Rosetta | Mach-O x86_64; SHA-256 `acb98063a9b95ed015a289918ecfd956d26b5085ab9ef6a78d4536a2a769eb1b` |
| `x86_64-unknown-linux-gnu` | Pass with the Zig cross linker | Not run on Linux | ELF x86-64; SHA-256 `7d6be12196b0baea76a2118948f914eebe807ca4e0b5e0d7c31bad87e938f40a` |
| `aarch64-unknown-linux-gnu` | Pass in an ARM64 Linux container | Pass in the same container | ELF AArch64; SHA-256 `c4687edd8619e2e9cb04cf639d86e1b8532485f9c29e08bdfc3ec10866282258` |

The four builds used V8 150.4.0 with the maintained macro package. The native
AArch64 Linux build completed with Rust 1.98.1 and the archive digest listed
above.
`make verify-engine-deps` reports no advisory, license, source or ban error.
Linux x86_64 was fully linked through a cross
linker on macOS; execution on Linux x86_64 was outside this build criterion.
