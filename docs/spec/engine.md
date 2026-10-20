<!-- doc-id: engine -->
[Korean](engine.ko.md)

# Engine verification

## Versions and local inputs

The workspace pins `deno_core =0.412.0` and `v8 =150.4.0` in `[workspace.dependencies]` of
`Cargo.toml`; `polyspec-ssr-runtime` and `polyspec-ssr-build` use `deno_core`. The committed
lockfile resolves `deno_v8 0.4.0` and `v8 150.4.0`.

`deno_v8 0.4.0` selects `v8 150.4.0` through its default `v8` feature. The
[`v8` build script](https://docs.rs/crate/v8/150.4.0/source/build.rs) obtains a
prebuilt archive from
`https://github.com/denoland/rusty_v8/releases/download/v150.4.0/`.
The selected features include `simdutf`, so the archive name is
`librusty_v8_simdutf_release_<target>.a.gz`. The `RUSTY_V8_ARCHIVE` environment
variable selects an absolute local archive path. The build script accepts this
path without checking SHA-256; `scripts/verify-archive.mts` verifies the compressed
archive and matching generated binding before invoking Cargo. It checks the exact V8 version
and feature set from offline locked Cargo metadata for the selected target. The make targets
that link V8, such as `make check` and `make bench`, reject source-build settings
and select only verified local inputs. Direct Cargo commands require the same input verification
and explicit archive and binding environment variables before execution.

The workspace uses the `v8 150.4.0` crate of crates.io with the
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

Linux is verified on the Linux runners of GitHub CI ([workspace](../operations/workspace.md)). On
`ubuntu-24.04-arm` and `ubuntu-24.04`, `make ci-setup` fetches the official archive and binding of
the runner's target and verifies their SHA-256 (`scripts/fetch-v8.mts`); the jobs `lint` and `test`
of `.github/workflows/ci.yml` then run Clippy on every target of the workspace (`check-clippy`),
build the `development_process` and `socket_process` examples (`check-examples`) and run every
test of the workspace (`ci-nextest`), which links `v8 150.4.0` and executes the engine on AArch64
and x86_64 Linux. macOS is verified by the tests of the workspace on the host (`make check`).

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
concurrently in separate processes.
