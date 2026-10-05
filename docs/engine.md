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

The V8 source checkout keeps version 150.4.0 and selects `pastey 0.2.3` as
the compile-time `paste` macro dependency. The previous macro package has a
maintenance advisory and no patched release. The replacement changes the
dependency package, not the V8 Rust macro calls or the prebuilt archive.
The engine verification workspace uses the absolute local V8 checkout. Engine
verification checks all four target builds and links after the dependency change.

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

`make verify-engine-linux-arm64` builds an ARM64 Linux container image from the
Rust 1.98.1 Bookworm image digest
`sha256:5b993f23fb69746405496e76f91e511d96c82b5b23304fd5d20b4a80a8b223ea`.
It requires the verified local archive and matching binding before mounting this
checkout read only at `/src`. It bind mounts the host's ignored
`var/engine-cargo` and `var/engine-target` directories at `/cargo` and `/target`
with write access, builds the program and executes it. `make` supplies absolute
host paths in the ignored `var/engine-compose.yaml` file so that later
`containerctl status` calls can read the same mount configuration. The host can inspect these directories, and
replacing the container preserves their contents. Each checkout has its own
stack: the Compose project is `ssr-engine-<digits>`, the container
`ssr-engine-<digits>-engine` and the image `localhost/ssr-engine-verify-<digits>:0.0.1`,
where the digits are the first 12 hexadecimal digits of the SHA-256 of the
checkout path. `tools/verify_engine_linux.py` runs the steps of
`make verify-engine-linux-arm64` and `make verify-engine-down` under the checkout
lock `var/locks/engine-verification.lock` of `tools/holder_lock.py`. The lock
record names the checkout, pid and process start time of its holder; a second
run is refused with that record, a lock whose holder no longer runs is
reported and kept until `python3 -m tools.holder_lock remove-stopped <lock file>`
removes it, and only the holder releases the lock. Each step prints its command,
its output and its result with the elapsed time, without a time limit. The native GNU linker
retains the Rust target's link arguments.
The local V8 checkout is also mounted read only at its absolute Cargo patch
path. The container cannot modify either source checkout.

## Renderer snapshots

One renderer process selects one immutable bundle key. The public snapshot creator initializes
its context once, serializes it and is completely disposed before worker isolates restore that
same blob. A different key is rejected even after all pools close. Different bundles use separate
processes. Request contexts remain independent and do not evaluate either bundle again.

The compiler uses the default V8 snapshot, so its isolate creation and disposal are coordinated
with renderer snapshot creation through `ssr_core::process`. A compiler cannot enter during
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
above. Mount inspection confirmed `/src` and the V8 checkout as read only and
`/cargo` and `/target` as writable host mounts. `containerctl status` read the
absolute Compose path without a project error. After replacing only the engine
container, the cached source and linked program remained visible on the host.
The repeated Cargo build compiled no crates and finished in 0.50 seconds; the
executable retained its SHA-256 digest and ran in the replacement container.
`make verify-engine-deps` reports no advisory, license, source or ban error.
No named volume is required. Linux x86_64 was fully linked through a cross
linker on macOS; execution on Linux x86_64 was outside this build criterion.
