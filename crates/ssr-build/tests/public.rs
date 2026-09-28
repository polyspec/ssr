use std::fs;
use std::os::unix::fs::{MetadataExt, symlink};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ssr_build::{BuildConfig, PublicFiles, build};

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

fn directory() -> PathBuf {
    let name = format!(
        "ssr-publish-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let dir = std::env::temp_dir().canonicalize().unwrap().join(name);
    fs::create_dir(&dir).unwrap();
    dir
}

#[tokio::test]
async fn publish_creates_public_files_preserves_equal_files_and_rejects_changes() {
    let build = build(&config()).await.unwrap();
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
    fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn publication_rejects_nonregular_targets_and_invalid_directory() {
    let build = build(&config()).await.unwrap();
    let public = PublicFiles::new(&build).unwrap();
    let dir = directory();
    let client = build.manifest.client.url.as_ref().unwrap();
    let target = dir.join(client.trim_start_matches('/'));
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::create_dir(&target).unwrap();
    assert!(public.publish(&dir).is_err());
    assert!(public.publish(Path::new("relative")).is_err());
    fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn publication_rejects_symbolic_link_parent_before_writing() {
    let build = build(&config()).await.unwrap();
    let public = PublicFiles::new(&build).unwrap();
    let dir = directory();
    let other = directory();
    symlink(&other, dir.join("assets")).unwrap();
    assert!(public.publish(&dir).is_err());
    assert_eq!(fs::read_dir(&other).unwrap().count(), 0);
    fs::remove_dir_all(dir).unwrap();
    fs::remove_dir_all(other).unwrap();
}

#[tokio::test]
async fn changed_target_prevents_other_public_files_from_being_created() {
    let build = build(&config()).await.unwrap();
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
    fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn serving_uses_exact_public_url_bytes_and_content_type() {
    let build = build(&config()).await.unwrap();
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
    let mut output = build(&config()).await.unwrap();
    output.manifest.client.sha256 = "invalid".into();
    assert!(PublicFiles::new(&output).is_err());

    let mut output = build(&config()).await.unwrap();
    output.manifest.client.url = Some("/assets/../outside.js".into());
    assert!(PublicFiles::new(&output).is_err());

    let mut output = build(&config()).await.unwrap();
    output.manifest.styles[0].url = output.manifest.client.url.clone();
    assert!(PublicFiles::new(&output).is_err());

    let mut output = build(&config()).await.unwrap();
    output.manifest.source_maps[0].url = Some("/assets/server.js.map".into());
    assert!(PublicFiles::new(&output).is_err());
}
