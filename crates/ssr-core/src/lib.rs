#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    #[test]
    fn core_has_no_local_crate_dependencies() {
        let manifest = include_str!("../Cargo.toml");
        for crate_name in [
            "ssr-build",
            "ssr-runtime",
            "ssr-adapter-react",
            "ssr-adapter-vue",
            "ssr-adapter-svelte",
            "ssr-adapter-vanilla",
            "ssr-server",
        ] {
            assert!(!manifest.contains(crate_name), "{crate_name}");
        }
    }
}
