use std::fs;
use std::path::PathBuf;

fn main() {
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .join("../..")
        .join("Cargo.toml");
    let source = fs::read_to_string(&manifest).expect("workspace manifest must be readable");
    let mut versions = source.lines().filter_map(|line| {
        line.strip_prefix("deno_core = \"=")
            .and_then(|version| version.strip_suffix('"'))
    });
    let version = versions
        .next()
        .expect("deno_core must have an exact version");
    assert!(versions.next().is_none(), "deno_core must have one version");
    assert!(
        !version.is_empty() && version.chars().all(|c| c.is_ascii_digit() || c == '.'),
        "deno_core version is invalid"
    );
    println!("cargo:rerun-if-changed={}", manifest.display());
    println!("cargo:rustc-env=SSR_DENO_CORE_VERSION={version}");
}
