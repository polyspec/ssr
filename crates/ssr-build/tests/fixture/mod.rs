//! The application root of one test.
//!
//! The nextest setup script `install-packages` (`tools/install_packages.py`) names the fixture
//! sources in `SSR_FIXTURES` and the immutable package installation of their lock in
//! `SSR_PACKAGES`. Each test copies the sources into a new temporary root of its own, writes its
//! generated entries there, builds with the installed packages as `BuildConfig::dependencies` and
//! removes the root when it ends, also when an assertion fails. No test writes into the checkout
//! or into the package installation, and two tests never share a root.
#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static NEXT: AtomicUsize = AtomicUsize::new(0);

pub struct Fixture {
    /// The application root of this test: a copy of the fixture sources.
    pub root: PathBuf,
    /// The `node_modules` directory of the package installation.
    pub packages: PathBuf,
}

fn variable(name: &str) -> PathBuf {
    let value = std::env::var_os(name).unwrap_or_else(|| {
        panic!(
            "{name} is not set; the nextest setup script install-packages sets it, so run the \
             tests with cargo nextest"
        )
    });
    let path = PathBuf::from(value);
    assert!(
        path.is_absolute() && path.is_dir(),
        "{name} must name an absolute directory: {}",
        path.display()
    );
    path
}

fn copy(source: &Path, destination: &Path) {
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let kind = entry.file_type().unwrap();
        let name = entry.file_name();
        let target = destination.join(&name);
        if kind.is_dir() {
            if name == "node_modules" {
                continue;
            }
            fs::create_dir(&target).unwrap();
            copy(&entry.path(), &target);
        } else {
            assert!(
                kind.is_file(),
                "fixture source is not a regular file: {}",
                entry.path().display()
            );
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

impl Fixture {
    pub fn new() -> Self {
        let sources = variable("SSR_FIXTURES");
        let packages = variable("SSR_PACKAGES").join("node_modules");
        assert!(
            packages.is_dir(),
            "SSR_PACKAGES holds no node_modules: {}",
            packages.display()
        );
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "ssr-fixture-{}-{nanos}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        fs::create_dir(&root).unwrap();
        let fixture = Self {
            root,
            packages: packages.canonicalize().unwrap(),
        };
        copy(&sources, &fixture.root);
        fixture
    }

    /// A directory of generated entries under the root.
    pub fn generated(&self, name: &str) -> PathBuf {
        let path = self.root.join(name);
        fs::create_dir(&path).unwrap();
        path
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}
