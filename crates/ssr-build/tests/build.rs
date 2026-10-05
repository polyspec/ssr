use std::fs;
use std::path::{Path, PathBuf};

use ordered_json::parse_bytes;
use sha2::{Digest, Sha256};
use ssr_build::{BuildConfig, build};

/// A temporary directory of one test, created new and removed when the test ends, also when an
/// assertion fails.
struct Temporary(PathBuf);

impl Temporary {
    fn new(prefix: &str) -> Self {
        let mut random = [0u8; 16];
        getrandom::fill(&mut random).unwrap();
        let name: String = random.iter().map(|byte| format!("{byte:02x}")).collect();
        let path = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("{prefix}-{name}"));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for Temporary {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/build-probe/tests/fixtures")
        .canonicalize()
        .unwrap()
}

fn config() -> BuildConfig {
    let root = fixture();
    BuildConfig {
        server_entry: root.join("server.tsx"),
        react_framework_entry: None,
        client_entry: root.join("client.tsx"),
        css_entry: root.join("app.css"),
        root,
        asset_route: "/assets".into(),
        dependencies: None,
    }
}

#[tokio::test]
async fn sample_build_has_stable_files_and_manifest() {
    let config = config();
    assert!(
        config.root.join("node_modules/react").is_dir(),
        "install sample packages before tests"
    );
    let first = build(&config).await.unwrap();
    let second = build(&config).await.unwrap();
    assert_eq!(
        first.files.keys().collect::<Vec<_>>(),
        second.files.keys().collect::<Vec<_>>()
    );
    for (path, bytes) in &first.files {
        assert_eq!(
            Sha256::digest(bytes),
            Sha256::digest(&second.files[path]),
            "{path}"
        );
    }
    assert_eq!(first.manifest, second.manifest);
    assert_eq!(
        first.manifest.to_json().unwrap(),
        second.manifest.to_json().unwrap()
    );
    let json = first.manifest.to_json().unwrap();
    let decoded = parse_bytes(&json).unwrap();
    assert_eq!(decoded.compact().as_bytes(), json);

    let manifest = &first.manifest;
    assert!(manifest.server.path.starts_with("server/server-"));
    assert!(manifest.client.path.starts_with("client/client-"));
    assert_eq!(manifest.server.url, None);
    assert!(
        manifest
            .client
            .url
            .as_ref()
            .unwrap()
            .starts_with("/assets/client-")
    );
    assert!(!manifest.styles.is_empty());
    assert!(!manifest.server_chunks.is_empty());
    assert!(!manifest.source_maps.is_empty());
    for javascript in std::iter::once(&manifest.server).chain(manifest.server_chunks.iter()) {
        let map_path = format!("{}.map", javascript.path);
        let source_map = manifest
            .source_maps
            .iter()
            .find(|map| map.path == map_path)
            .unwrap();
        assert!(source_map.url.is_none());
        let decoded = sourcemap::SourceMap::from_slice(&first.files[&map_path]).unwrap();
        assert!(decoded.get_token_count() > 0);
    }
    assert!(manifest.assets.iter().any(|a| a.path.contains("extra-")));
    let server = String::from_utf8(first.files[&manifest.server.path].clone()).unwrap();
    let client = String::from_utf8(first.files[&manifest.client.path].clone()).unwrap();
    assert!(server.contains("renderToString"));
    assert!(client.contains("createRoot"));
    let style = String::from_utf8(first.files[&manifest.styles[0].path].clone()).unwrap();
    assert!(style.contains("font-family: Inter"));
    for extension in [
        "png", "svg", "jpg", "gif", "webp", "avif", "ico", "woff", "woff2", "ttf",
    ] {
        let source = fs::read(config.root.join(format!("assets/sample.{extension}"))).unwrap();
        assert!(
            manifest
                .assets
                .iter()
                .any(|a| a.path.ends_with(&format!(".{extension}"))
                    && first.files[&a.path] == source),
            "missing {extension}"
        );
    }
    for asset in &manifest.assets {
        if asset.path.starts_with("client/assets/sample-")
            && !asset.path.contains(&asset.sha256[..16])
        {
            let url = asset.url.as_ref().expect("sample assets are public");
            assert!(client.contains(url), "client does not reference {url}");
            assert!(server.contains(url), "server does not reference {url}");
        }
    }
    for artifact in std::iter::once(&manifest.server)
        .chain(std::iter::once(&manifest.client))
        .chain(manifest.styles.iter())
        .chain(manifest.server_chunks.iter())
        .chain(manifest.source_maps.iter())
        .chain(manifest.assets.iter())
    {
        let bytes = &first.files[&artifact.path];
        let digest = Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        assert_eq!(artifact.sha256, digest, "{}", artifact.path);
        if artifact.path.contains(&artifact.sha256[..16])
            && (artifact.path.ends_with(".svg") || artifact.path.ends_with(".woff2"))
        {
            let url = artifact.url.as_ref().expect("CSS assets are public");
            assert!(style.contains(url));
        }
    }
    let listed_count = 2
        + manifest.styles.len()
        + manifest.server_chunks.len()
        + manifest.source_maps.len()
        + manifest.assets.len();
    assert_eq!(
        first.files.len(),
        listed_count,
        "every output must be listed"
    );
}

#[test]
fn a_failed_assertion_removes_the_temporary_directory() {
    let mut created = None;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let temporary = Temporary::new("ssr-build-failed-case");
        fs::write(temporary.0.join("file"), "content").unwrap();
        created = Some(temporary.0.clone());
        panic!("a failed assertion of the case");
    }));
    assert!(result.is_err());
    let created = created.unwrap();
    assert!(
        !created.exists(),
        "the directory of a failed case remains: {}",
        created.display()
    );
}

#[tokio::test]
async fn package_resolution_uses_the_application_dependency_directory() {
    let temporary = Temporary::new("ssr-build-package-resolution");
    let root = temporary.0.clone();
    fs::create_dir_all(root.join("src/node_modules/shared")).unwrap();
    fs::create_dir_all(root.join("node_modules/shared")).unwrap();
    fs::write(root.join("app.css"), "").unwrap();
    fs::write(
        root.join("node_modules/shared/package.json"),
        r#"{"type":"module","main":"index.js"}"#,
    )
    .unwrap();
    fs::write(
        root.join("node_modules/shared/index.js"),
        "export const marker = 'root-dependency';",
    )
    .unwrap();
    fs::write(
        root.join("src/node_modules/shared/package.json"),
        r#"{"type":"module","main":"index.js"}"#,
    )
    .unwrap();
    fs::write(
        root.join("src/node_modules/shared/index.js"),
        "export const marker = 'nested-dependency';",
    )
    .unwrap();
    fs::write(
        root.join("src/local.js"),
        "import {marker} from 'shared'; export const local = marker;",
    )
    .unwrap();
    fs::write(
        root.join("src/server.js"),
        "import {marker} from 'shared'; import {local} from './local.js'; export function render() { return {html: marker + local, head: '', state: null}; }",
    )
    .unwrap();
    fs::write(
        root.join("client.js"),
        "import {marker} from 'shared'; document.body.dataset.marker = marker;",
    )
    .unwrap();
    let root = root.canonicalize().unwrap();
    let output = build(&BuildConfig {
        root: root.clone(),
        server_entry: root.join("src/server.js"),
        react_framework_entry: None,
        client_entry: root.join("client.js"),
        css_entry: root.join("app.css"),
        asset_route: "/assets".into(),
        dependencies: None,
    })
    .await
    .unwrap();
    let server = String::from_utf8(output.files[&output.manifest.server.path].clone()).unwrap();
    assert!(server.contains("root-dependency"));
    assert!(!server.contains("nested-dependency"));
}

#[tokio::test]
async fn invalid_entries_imports_and_route_fail() {
    let mut config = config();
    config.react_framework_entry = Some(config.server_entry.clone());
    assert!(build(&config).await.is_err());
    config.react_framework_entry = Some(config.root.join("missing-framework.tsx"));
    assert!(build(&config).await.is_err());
    config.react_framework_entry = None;
    config.asset_route = "//external".into();
    assert!(build(&config).await.is_err());
    config.asset_route = "/assets".into();
    config.server_entry = config.root.join("absent.tsx");
    assert!(build(&config).await.is_err());
    config.server_entry = config.root.join("server.tsx");
    config.client_entry = config.root.join("missing.tsx");
    assert!(build(&config).await.is_err());
    config.client_entry = config.root.join("client.tsx");
    config.css_entry = config.root.join("missing.css");
    assert!(build(&config).await.is_err());
    config.css_entry = config.root.join("missing-url.css");
    assert!(build(&config).await.is_err());
    config.css_entry = config.root.join("app.css");
    config.client_entry = config.root.join("query.tsx");
    assert!(build(&config).await.is_err());
}

#[tokio::test]
async fn root_asset_route_has_one_leading_slash() {
    let mut config = config();
    config.asset_route = "/".into();
    let output = build(&config).await.unwrap();
    let client = String::from_utf8(output.files[&output.manifest.client.path].clone()).unwrap();
    for asset in &output.manifest.assets {
        if asset.path.starts_with("client/assets/sample-")
            && !asset.path.contains(&asset.sha256[..16])
        {
            let url = asset.url.as_ref().unwrap();
            assert!(url.starts_with("/assets/"));
            assert!(!url.starts_with("//"));
            assert!(client.contains(url));
        }
    }
}

#[tokio::test]
async fn configured_dependencies_replace_nested_node_modules_packages() {
    let temporary = Temporary::new("ssr-build-deps");
    let root = temporary.0.clone();
    let nested = root.join("node_modules/shared-value");
    let dependencies = root.join("dependencies");
    let configured = dependencies.join("shared-value");
    fs::create_dir_all(&nested).unwrap();
    fs::create_dir_all(&configured).unwrap();
    for (directory, marker) in [(&nested, "nested-copy"), (&configured, "dependency-copy")] {
        fs::write(
            directory.join("package.json"),
            r#"{"name":"shared-value","main":"index.js"}"#,
        )
        .unwrap();
        fs::write(
            directory.join("index.js"),
            format!("export const marker = '{marker}';"),
        )
        .unwrap();
    }
    let root = root.canonicalize().unwrap();
    let entry = root.join("entry.js");
    fs::write(
        &entry,
        "import { marker } from 'shared-value'; export function render(props, state) { return {head:'', html:'<p>' + marker + '</p>', state}; }",
    )
    .unwrap();
    let css = root.join("app.css");
    fs::write(&css, "main{color:red}").unwrap();
    let client = root.join("client.js");
    fs::write(&client, "console.log('client');").unwrap();
    let output = build(&BuildConfig {
        root: root.clone(),
        server_entry: entry,
        react_framework_entry: None,
        client_entry: client,
        css_entry: css,
        asset_route: "/assets".into(),
        dependencies: Some(dependencies),
    })
    .await
    .unwrap();
    let server = String::from_utf8(output.files[&output.manifest.server.path].clone()).unwrap();
    assert!(
        server.contains("dependency-copy"),
        "the nested node_modules copy was bundled: {server}"
    );
    assert!(
        !server.contains("nested-copy"),
        "the nested node_modules copy was bundled: {server}"
    );
}

/// An application whose CSS and JavaScript import a configured dependency package that ships a
/// font and an image, with the package directory outside the application root.
fn package_asset_application(temporary: &Temporary) -> BuildConfig {
    let root = temporary.0.join("application");
    let dependencies = temporary.0.join("dependencies");
    let package = dependencies.join("asset-package");
    fs::create_dir_all(package.join("files")).unwrap();
    fs::create_dir(&root).unwrap();
    fs::write(
        package.join("package.json"),
        r#"{"name":"asset-package","main":"index.js"}"#,
    )
    .unwrap();
    fs::write(package.join("files/font.woff2"), b"font").unwrap();
    fs::write(
        package.join("files/mark.svg"),
        "<svg xmlns=\"http://www.w3.org/2000/svg\"/>",
    )
    .unwrap();
    fs::write(
        package.join("style.css"),
        "@font-face{font-family:F;src:url(\"./files/font.woff2\")}",
    )
    .unwrap();
    fs::write(
        package.join("index.js"),
        "import mark from './files/mark.svg'; export const marker = mark;",
    )
    .unwrap();
    fs::write(root.join("app.css"), "@import \"asset-package/style.css\";").unwrap();
    fs::write(
        root.join("entry.js"),
        "import { marker } from 'asset-package'; export function render(props, state) { return {head:'', html: marker, state}; }",
    )
    .unwrap();
    fs::write(
        root.join("client.js"),
        "import { marker } from 'asset-package'; console.log(marker);",
    )
    .unwrap();
    BuildConfig {
        server_entry: root.join("entry.js"),
        react_framework_entry: None,
        client_entry: root.join("client.js"),
        css_entry: root.join("app.css"),
        root,
        asset_route: "/assets".into(),
        dependencies: Some(dependencies),
    }
}

#[tokio::test]
async fn assets_of_a_configured_dependency_package_are_built() {
    let temporary = Temporary::new("ssr-build-package-assets");
    let output = build(&package_asset_application(&temporary)).await.unwrap();
    let names: Vec<_> = output.files.keys().cloned().collect();
    assert!(
        names
            .iter()
            .any(|name| name.contains("font-") && name.ends_with(".woff2")),
        "{names:?}"
    );
    assert!(
        names
            .iter()
            .any(|name| name.contains("mark-") && name.ends_with(".svg")),
        "{names:?}"
    );
}

#[tokio::test]
async fn assets_outside_the_root_and_the_package_directory_are_rejected() {
    let temporary = Temporary::new("ssr-build-outside-assets");
    let config = package_asset_application(&temporary);
    let outside = temporary.0.join("outside");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("font.woff2"), b"font").unwrap();
    fs::write(
        outside.join("mark.svg"),
        "<svg xmlns=\"http://www.w3.org/2000/svg\"/>",
    )
    .unwrap();
    let package = temporary.0.join("dependencies/asset-package");
    fs::write(
        package.join("style.css"),
        "@font-face{font-family:F;src:url(\"../../outside/font.woff2\")}",
    )
    .unwrap();
    let error = build(&config).await.unwrap_err().to_string();
    assert!(
        error.contains("CSS asset outside application root and package directory"),
        "{error}"
    );
    fs::write(package.join("style.css"), "p{color:red}").unwrap();
    fs::write(
        package.join("index.js"),
        "import mark from '../../outside/mark.svg'; export const marker = mark;",
    )
    .unwrap();
    let error = build(&config).await.unwrap_err().to_string();
    assert!(
        error.contains("Could not load ../outside/mark.svg")
            && error.contains("plugin `ssr-build-assets` threw an error"),
        "{error}"
    );
}

#[path = "exports.rs"]
mod exports;
