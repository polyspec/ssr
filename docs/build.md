[Korean](build.ko.md)

# Build verification

## Acceptance

The tracked sample must use rolldown 1.2.11 through its Rust API to build separate server and
client entries written in React TSX. Both builds must emit content-hashed JavaScript names. A
dynamic import must produce a separate chunk. The client bundle must contain React code. The
server bundle must contain React rendering code.

The sample must use lightningcss 1.0.0-alpha.72 to combine a CSS entry and a package stylesheet
imported from `node_modules`. Relative `url()` references in both sheets must resolve to their
source files and refer to emitted hashed assets. The sample must verify emitted files for png, svg,
jpg, gif, webp, avif, ico, woff, woff2 and ttf with their source bytes and content-hashed names.
An unresolved import or asset must fail the build.

Run `make verify-build` to install the pinned sample packages, run the verification tests and
check the probe dependencies. The probe is a separate Cargo workspace under `tools/build-probe`.
It uses a checkout of the CSS library because its bundler feature separates source locations
from source-map generation. The probe manifest names
that checkout by its path, which only the verification probe uses. The product workspace has no
build dependency until S-5, when that crate must be connected to the product build.

## Result and API

The sample passed on macOS aarch64 with Rust 1.98.1. `Bundler::new(BundlerOptions)` and
`Bundler::generate().await` build both React TSX entries. Set `input` to an `InputItem` for each
entry, `platform` to `Browser`, `format` to `Esm`, `code_splitting` to `Bool(true)`, and
`entry_filenames` and `chunk_filenames` to `[name]-[hash].js`. Set `asset_filenames` to
`assets/[name]-[hash][extname]` and map each listed extension to `ModuleType::Asset`. The
verification checks the entry chunks contain React client or server rendering code, the dynamic
import emits a separate chunk, and all ten asset types retain their source bytes. The test also
checks that missing JavaScript assets fail. Asset samples use opaque bytes because this check
verifies the build's file handling, not image or font decoding.

`lightningcss::bundler::Bundler::new` bundles CSS with a `SourceProvider` that resolves package
imports from `node_modules`. The built-in `FileProvider` resolves imports relative to the
originating file only, so a package import needs the custom provider. Call `StyleSheet::to_css`
with `PrinterOptions::analyze_dependencies` to obtain `url()` locations and placeholders. The
caller reads each URL relative to its source stylesheet, writes the asset under a SHA-256 name,
and replaces its placeholder with the public URL. The sample verifies the local SVG and package
WOFF2 references and checks that a missing CSS import fails. Lightning CSS does not emit files or
replace placeholders with final URLs; this explicit asset step is the required replacement.

`make verify-build` runs `npm ci` before the Rust tests. Bundling and CSS processing then run in
Rust with no Node process. The sample does not test browser image or font decoding, and its asset
bytes are not media fixtures. Both sample tests and the dependency check pass.

## Dependency result

The bundled CSS API originally required source-map support and thus
[RUSTSEC-2026-0235](https://rustsec.org/advisories/RUSTSEC-2026-0235) through rkyv 0.7.46.
The CSS library now makes source-map support optional for bundling. Its source locations still
identify the original stylesheet for CSS module names and `url()` references. The probe enables
`bundler` without `sourcemap`; its lockfile contains neither parcel_sourcemap nor rkyv. The CSS
library's tests pass with and without source-map support (120 and 117 tests respectively).
`make verify-build` passes both sample tests and all cargo-deny checks. The required Rolldown
graph uses xxhash-rust 0.8.18 and dragonbox_ecma 0.1.12; exact-version license exceptions are
recorded in [workspace.md](workspace.md).
The clean build also passes after removing the probe's Cargo target files and reinstalling the
sample packages.

Rolldown 1.2.11 rejects CSS modules with an explicit unsupported feature error. The selected
replacement for CSS is Lightning CSS's bundler with a package-aware `SourceProvider` and explicit
asset URL replacement. The present build does not request CSS source maps. Enabling the CSS
`sourcemap` feature adds `parcel_sourcemap` and `rkyv` to that caller's dependency graph, so that
graph requires its own advisory check before use.
