mod fixture;
use std::fs;
use std::os::unix::fs::{MetadataExt, symlink};
use std::path::{Path, PathBuf};

use ssr_build::{BuildConfig, PublicFiles, build};

fn config(fixture: &fixture::Fixture) -> BuildConfig {
    let root = fixture.root.clone();
    BuildConfig {
        server_entry: root.join("server.tsx"),
        react_framework_entry: None,
        client_entry: root.join("client.tsx"),
        css_entry: root.join("app.css"),
        root,
        asset_route: "/assets".into(),
        dependencies: Some(fixture.packages.clone()),
    }
}

/// The sample build of a fixture root of its own.
async fn sample() -> ssr_build::Build {
    let fixture = fixture::Fixture::new();
    build(&config(&fixture)).await.unwrap()
}

/// A publication directory of one test, created new and removed when the test ends, also when an
/// assertion fails.
struct Directory(PathBuf);

impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

impl std::ops::Deref for Directory {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}

impl AsRef<Path> for Directory {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

fn directory() -> Directory {
    let mut random = [0u8; 16];
    getrandom::fill(&mut random).unwrap();
    let name: String = random.iter().map(|byte| format!("{byte:02x}")).collect();
    let dir = std::env::temp_dir()
        .canonicalize()
        .unwrap()
        .join(format!("ssr-publish-{name}"));
    fs::create_dir(&dir).unwrap();
    Directory(dir)
}

#[tokio::test]
async fn publish_creates_public_files_preserves_equal_files_and_rejects_changes() {
    let build = sample().await;
    let public = PublicFiles::new(&build).unwrap();
    let dir = directory();
    public.publish(&dir).unwrap();

    let client = &build.manifest.client;
    let client_path = dir.join(client.url.as_ref().unwrap().trim_start_matches('/'));
    assert_eq!(fs::read(&client_path).unwrap(), build.files[&client.path]);
    assert!(!dir.join(&build.manifest.server.path).exists());
    for chunk in &build.manifest.server_chunks {
        assert!(!dir.join(&chunk.path).exists());
    }
    for map in &build.manifest.source_maps {
        assert!(!dir.join(&map.path).exists());
        assert!(map.url.is_none());
    }

    let original_inode = fs::metadata(&client_path).unwrap().ino();
    public.publish(&dir).unwrap();
    assert_eq!(fs::metadata(&client_path).unwrap().ino(), original_inode);

    fs::write(&client_path, b"different bytes").unwrap();
    assert!(public.publish(&dir).is_err());
    assert_eq!(fs::read(&client_path).unwrap(), b"different bytes");
}

#[tokio::test]
async fn publication_rejects_nonregular_targets_and_invalid_directory() {
    let build = sample().await;
    let public = PublicFiles::new(&build).unwrap();
    let dir = directory();
    let client = build.manifest.client.url.as_ref().unwrap();
    let target = dir.join(client.trim_start_matches('/'));
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::create_dir(&target).unwrap();
    assert!(public.publish(&dir).is_err());
    assert!(public.publish(Path::new("relative")).is_err());
}

#[tokio::test]
async fn publication_rejects_symbolic_link_parent_before_writing() {
    let build = sample().await;
    let public = PublicFiles::new(&build).unwrap();
    let dir = directory();
    let other = directory();
    symlink(&other, dir.join("assets")).unwrap();
    assert!(public.publish(&dir).is_err());
    assert_eq!(fs::read_dir(&other).unwrap().count(), 0);
}

#[tokio::test]
async fn changed_target_prevents_other_public_files_from_being_created() {
    let build = sample().await;
    let public = PublicFiles::new(&build).unwrap();
    let dir = directory();
    let target = dir.join(
        build
            .manifest
            .client
            .url
            .as_ref()
            .unwrap()
            .trim_start_matches('/'),
    );
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(&target, b"different bytes").unwrap();
    assert!(public.publish(&dir).is_err());
    assert_eq!(fs::read_dir(target.parent().unwrap()).unwrap().count(), 1);
}

#[tokio::test]
async fn serving_uses_exact_public_url_bytes_and_content_type() {
    let build = sample().await;
    let public = PublicFiles::new(&build).unwrap();
    for artifact in std::iter::once(&build.manifest.client)
        .chain(build.manifest.styles.iter())
        .chain(build.manifest.assets.iter())
    {
        let served = public.get(artifact.url.as_ref().unwrap()).unwrap();
        assert_eq!(served.bytes(), build.files[&artifact.path]);
        assert_eq!(served.content_type(), artifact.content_type);
    }
    assert!(public.get(&build.manifest.server.path).is_none());
    assert!(public.get("/assets/../server/server.js").is_none());
    assert!(public.get("/assets/absent.js").is_none());
}

#[tokio::test]
async fn public_files_reject_invalid_manifest_or_bytes() {
    let mut output = sample().await;
    output.manifest.client.sha256 = "invalid".into();
    assert!(PublicFiles::new(&output).is_err());

    let mut output = sample().await;
    output.manifest.client.url = Some("/assets/../outside.js".into());
    assert!(PublicFiles::new(&output).is_err());

    let mut output = sample().await;
    output.manifest.styles[0].url = output.manifest.client.url.clone();
    assert!(PublicFiles::new(&output).is_err());

    let mut output = sample().await;
    output.manifest.source_maps[0].url = Some("/assets/server.js.map".into());
    assert!(PublicFiles::new(&output).is_err());
}
