use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::{MetadataExt, symlink};
use std::path::PathBuf;
use std::sync::{Arc, Barrier};

use sha2::{Digest, Sha256};
use ssr_build::{Build, BuildFile, Manifest, PublicFiles};

fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

struct Directory(PathBuf);

impl Directory {
    fn new() -> Self {
        let mut random = [0u8; 16];
        getrandom::fill(&mut random).unwrap();
        let path = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("ssr-build-{}", digest(&random)));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn fixture() -> Build {
    let mut files = BTreeMap::new();
    let mut file = |path: &str, url: Option<&str>, content_type: &str| {
        let bytes = path.as_bytes().to_vec();
        files.insert(path.to_string(), bytes.clone());
        BuildFile {
            path: path.into(),
            url: url.map(String::from),
            sha256: digest(&bytes),
            content_type: content_type.into(),
        }
    };
    let js = "text/javascript; charset=utf-8";
    let map = "application/json; charset=utf-8";
    let server = file("server/app.js", None, js);
    let framework = file("server/react.js", None, js);
    let source_maps = vec![
        file("server/app.js.map", None, map),
        file("server/react.js.map", None, map),
    ];
    let client = file("client/app.js", Some("/assets/app.js"), js);
    let styles = vec![file(
        "client/app.css",
        Some("/assets/app.css"),
        "text/css; charset=utf-8",
    )];
    let assets = vec![file(
        "client/icon.svg",
        Some("/assets/icon.svg"),
        "image/svg+xml",
    )];
    Build {
        manifest: Manifest {
            server,
            react_framework: Some(framework),
            source_maps,
            client,
            styles,
            server_chunks: Vec::new(),
            assets,
        },
        files,
    }
}

#[test]
fn complete_build_round_trips_and_equal_writes_preserve_files() {
    let dir = Directory::new();
    let build = fixture();
    let output = build.write(&dir.0).unwrap();
    assert_eq!(
        output,
        dir.0.join(digest(&build.manifest.to_json().unwrap()))
    );
    let before = fs::metadata(output.join("manifest.json")).unwrap().ino();
    let read = Build::read(&output).unwrap();
    assert_eq!(read.manifest, build.manifest);
    assert_eq!(read.files, build.files);
    assert_eq!(build.write(&dir.0).unwrap(), output);
    assert_eq!(
        fs::metadata(output.join("manifest.json")).unwrap().ino(),
        before
    );
    let public = PublicFiles::new(&read).unwrap();
    assert!(public.get("/assets/app.js").is_some());
    assert!(public.get("/server/app.js").is_none());
    assert!(public.get("/server/react.js").is_none());
    assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 1);
}

#[test]
fn different_build_preserves_previous_generation() {
    let dir = Directory::new();
    let first = fixture();
    let old = first.write(&dir.0).unwrap();
    let mut second = fixture();
    second
        .files
        .insert(second.manifest.server.path.clone(), b"changed".to_vec());
    second.manifest.server.sha256 = digest(b"changed");
    let new = second.write(&dir.0).unwrap();
    assert_ne!(old, new);
    assert_eq!(Build::read(&old).unwrap().files, first.files);
    assert_eq!(Build::read(&new).unwrap().files, second.files);
}

#[test]
fn failed_directory_write_removes_partial_files_and_preserves_previous_build() {
    let dir = Directory::new();
    let original = fixture();
    let output = original.write(&dir.0).unwrap();
    let mut changed = fixture();
    for file in changed.manifest.react_framework.iter_mut().chain(
        changed
            .manifest
            .source_maps
            .iter_mut()
            .filter(|file| file.path == "server/react.js.map"),
    ) {
        let bytes = changed.files.remove(&file.path).unwrap();
        file.path = file.path.replace("server/react", "server/app.js/react");
        changed.files.insert(file.path.clone(), bytes);
    }
    assert!(changed.write(&dir.0).is_err());
    assert_eq!(Build::read(&output).unwrap().files, original.files);
    assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 1);
}

#[test]
fn invalid_build_fails_before_publication() {
    for mutate in [
        |build: &mut Build| {
            build.manifest.server.sha256 = "bad".into();
        },
        |build: &mut Build| {
            build.manifest.server.path = "../outside.js".into();
        },
        |build: &mut Build| {
            build.manifest.server.url = Some("/assets/app.js".into());
        },
        |build: &mut Build| {
            build.manifest.react_framework.as_mut().unwrap().url = Some("/assets/react.js".into());
        },
        |build: &mut Build| {
            build.manifest.client.url = None;
        },
        |build: &mut Build| {
            build.manifest.client.content_type = "text/plain".into();
        },
        |build: &mut Build| {
            build.manifest.assets.push(build.manifest.client.clone());
        },
        |build: &mut Build| {
            build.files.insert("extra.js".into(), vec![]);
        },
        |build: &mut Build| {
            build.files.remove(&build.manifest.server.path);
        },
        |build: &mut Build| {
            build.manifest.source_maps.clear();
        },
    ] {
        let dir = Directory::new();
        let mut build = fixture();
        mutate(&mut build);
        assert!(build.write(&dir.0).is_err());
        assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 0);
    }
}

#[test]
fn read_rejects_changed_missing_extra_and_symbolic_files() {
    for mutate in [
        |path: &PathBuf| {
            fs::write(path.join("server/app.js"), b"changed").unwrap();
        },
        |path: &PathBuf| {
            fs::remove_file(path.join("server/app.js")).unwrap();
        },
        |path: &PathBuf| {
            fs::write(path.join("extra.txt"), b"extra").unwrap();
        },
        |path: &PathBuf| {
            fs::create_dir(path.join("extra")).unwrap();
        },
        |path: &PathBuf| {
            fs::rename(path.join("server/app.js"), path.join("server/source.js")).unwrap();
            symlink(path.join("server/source.js"), path.join("server/app.js")).unwrap();
        },
    ] {
        let dir = Directory::new();
        let build = fixture();
        let output = build.write(&dir.0).unwrap();
        mutate(&output);
        assert!(Build::read(&output).is_err());
        assert!(build.write(&dir.0).is_err());
    }
}

#[test]
fn read_rejects_duplicate_unknown_and_invalid_manifest_fields() {
    for mutate in [
        |json: String| json.replacen('{', "{\"extra\":null,", 1),
        |json: String| json.replacen('{', "{\"server\":null,", 1),
        |json: String| json.replacen("\"styles\":[", "\"styles\":null,\"removed\":[", 1),
        |json: String| json.replacen("\"path\":", "\"extra\":1,\"path\":", 1),
    ] {
        let dir = Directory::new();
        let output = fixture().write(&dir.0).unwrap();
        let path = output.join("manifest.json");
        fs::write(&path, mutate(fs::read_to_string(&path).unwrap())).unwrap();
        assert!(Build::read(&output).is_err());
    }
}

#[test]
fn read_rejects_changed_manifest_bytes_even_when_fields_are_unchanged() {
    let dir = Directory::new();
    let output = fixture().write(&dir.0).unwrap();
    let manifest = output.join("manifest.json");
    let mut bytes = fs::read(&manifest).unwrap();
    bytes.push(b'\n');
    fs::write(&manifest, bytes).unwrap();
    assert!(Build::read(&output).is_err());
}

#[test]
fn read_and_write_reject_relative_and_symbolic_directories() {
    let dir = Directory::new();
    let build = fixture();
    assert!(build.write(std::path::Path::new("relative")).is_err());
    assert!(Build::read(std::path::Path::new("relative")).is_err());
    let output = build.write(&dir.0).unwrap();
    let link = dir.0.join("link");
    symlink(&output, &link).unwrap();
    assert!(Build::read(&link).is_err());
    assert!(build.write(&link).is_err());
    fs::rename(&output, dir.0.join("wrong-digest")).unwrap();
    assert!(Build::read(&dir.0.join("wrong-digest")).is_err());
}

#[test]
fn concurrent_equal_publications_return_one_complete_directory() {
    let dir = Directory::new();
    let barrier = Arc::new(Barrier::new(4));
    let outputs = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let barrier = barrier.clone();
                let directory = &dir.0;
                scope.spawn(move || {
                    barrier.wait();
                    let output = fixture().write(directory).unwrap();
                    assert_eq!(Build::read(&output).unwrap().files, fixture().files);
                    output
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert!(outputs.iter().all(|output| output == &outputs[0]));
    assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 1);
}

#[tokio::test]
async fn generated_bundle_files_are_read_without_rebuilding() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/build-probe/tests/fixtures")
        .canonicalize()
        .unwrap();
    let build = ssr_build::build(&ssr_build::BuildConfig {
        server_entry: root.join("server.tsx"),
        react_framework_entry: None,
        client_entry: root.join("client.tsx"),
        css_entry: root.join("app.css"),
        root,
        asset_route: "/assets".into(),
    })
    .await
    .unwrap();
    assert!(!build.manifest.server_chunks.is_empty());
    let dir = Directory::new();
    let output = build.write(&dir.0).unwrap();
    let loaded = Build::read(&output).unwrap();
    assert_eq!(loaded.manifest, build.manifest);
    assert_eq!(loaded.files, build.files);
    let public = PublicFiles::new(&loaded).unwrap();
    assert_eq!(
        public
            .get(loaded.manifest.client.url.as_ref().unwrap())
            .unwrap()
            .bytes(),
        loaded.files[&loaded.manifest.client.path]
    );
}
