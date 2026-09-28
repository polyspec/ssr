#![forbid(unsafe_code)]

mod asset_url;
mod css;
mod files;
mod js;
mod manifest;
mod manifest_read;
mod public;
mod react_css;
mod svelte;

pub use manifest::{BuildFile, Manifest};
pub use public::{PublicFile, PublicFiles, PublishError};

use std::collections::BTreeMap;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

#[derive(Debug, Clone)]
pub struct BuildConfig {
    pub root: PathBuf,
    pub server_entry: PathBuf,
    pub react_framework_entry: Option<PathBuf>,
    pub client_entry: PathBuf,
    pub css_entry: PathBuf,
    pub asset_route: String,
    /// The directory every application package import resolves from. When it
    /// is set, packages under a `node_modules` directory of the application
    /// source are never used, so one package has one module instance.
    pub dependencies: Option<PathBuf>,
}

impl BuildConfig {
    /// The single directory package imports resolve from.
    pub fn package_directory(&self) -> PathBuf {
        self.dependencies
            .clone()
            .unwrap_or_else(|| self.root.join("node_modules"))
    }
}

#[derive(Debug)]
pub struct Build {
    pub manifest: Manifest,
    pub files: BTreeMap<String, Vec<u8>>,
}

#[derive(Debug)]
pub enum Error {
    InvalidInput(String),
    Io(io::Error),
    JavaScript(String),
    Css(String),
    Manifest(ordered_json::Error),
    DuplicateOutput(String),
    Random(getrandom::Error),
    Cleanup {
        operation: Box<Self>,
        cleanup: io::Error,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message) => write!(f, "invalid build input: {message}"),
            Self::Io(error) => write!(f, "build I/O failed: {error}"),
            Self::JavaScript(message) => write!(f, "JavaScript build failed: {message}"),
            Self::Css(message) => write!(f, "CSS build failed: {message}"),
            Self::Manifest(error) => write!(f, "manifest failed: {error}"),
            Self::DuplicateOutput(path) => write!(f, "duplicate build output: {path}"),
            Self::Random(error) => write!(f, "build random value failed: {error}"),
            Self::Cleanup { operation, cleanup } => write!(
                f,
                "{operation}; temporary directory removal failed: {cleanup}"
            ),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Manifest(error) => Some(error),
            Self::Random(error) => Some(error),
            Self::Cleanup { operation, .. } => Some(operation),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<ordered_json::Error> for Error {
    fn from(error: ordered_json::Error) -> Self {
        Self::Manifest(error)
    }
}

pub(crate) fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub(crate) fn public_url(route: &str, filename: &str) -> String {
    if route == "/" {
        format!("/{filename}")
    } else {
        format!("{route}/{filename}")
    }
}

fn validate_entry(root: &Path, path: &Path) -> Result<(), Error> {
    if !path.is_absolute() || !path.starts_with(root) || !path.is_file() {
        return Err(Error::InvalidInput(format!(
            "entry must be a file under application root: {}",
            path.display()
        )));
    }
    if path.canonicalize()? != path {
        return Err(Error::InvalidInput(format!(
            "entry must not contain symbolic links: {}",
            path.display()
        )));
    }
    Ok(())
}

fn validate(config: &BuildConfig) -> Result<(), Error> {
    if !config.root.is_absolute()
        || !config.root.is_dir()
        || config.root.canonicalize()? != config.root
    {
        return Err(Error::InvalidInput(
            "root must be an absolute directory without symbolic links".into(),
        ));
    }
    for path in [
        &config.server_entry,
        &config.client_entry,
        &config.css_entry,
    ] {
        validate_entry(&config.root, path)?;
    }
    if let Some(path) = &config.react_framework_entry {
        validate_entry(&config.root, path)?;
        if path == &config.server_entry {
            return Err(Error::InvalidInput(
                "React framework and application entries must differ".into(),
            ));
        }
    }
    let route = &config.asset_route;
    if !route.starts_with('/')
        || (route != "/" && route.ends_with('/'))
        || route.contains("//")
        || (route != "/"
            && route.split('/').skip(1).any(|part| {
                part.is_empty()
                    || part == "."
                    || part == ".."
                    || !part.bytes().all(|byte| {
                        byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.')
                    })
            }))
    {
        return Err(Error::InvalidInput(format!("invalid asset route: {route}")));
    }
    Ok(())
}

impl Build {
    pub(crate) fn insert(
        &mut self,
        path: String,
        bytes: Vec<u8>,
        public_route: Option<&str>,
    ) -> Result<BuildFile, Error> {
        if let Some(existing) = self.files.get(&path) {
            if existing != &bytes {
                return Err(Error::DuplicateOutput(path));
            }
            return BuildFile::new(&path, &bytes, public_route);
        }
        let artifact = BuildFile::new(&path, &bytes, public_route)?;
        self.files.insert(path, bytes);
        Ok(artifact)
    }
}

pub async fn build(config: &BuildConfig) -> Result<Build, Error> {
    validate(config)?;
    let mut result = Build {
        manifest: Manifest::empty(),
        files: BTreeMap::new(),
    };
    if let Some(entry) = &config.react_framework_entry {
        let styles = js::bundle(config, entry, "react_framework", &mut result).await?;
        if !styles.is_empty() {
            return Err(Error::JavaScript(
                "React framework bundle contains Svelte styles".into(),
            ));
        }
    }
    let server_styles = js::bundle(config, &config.server_entry, "server", &mut result).await?;
    let client_styles = js::bundle(config, &config.client_entry, "client", &mut result).await?;
    if server_styles != client_styles {
        return Err(Error::Css("server and client Svelte styles differ".into()));
    }
    for (source, css) in client_styles {
        let style = css::bundle_component(config, &mut result, &source, css)?;
        result.manifest.styles.push(style);
    }
    let style = css::bundle(config, &mut result)?;
    result.manifest.styles.push(style);
    result.manifest.sort();
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::{Build, Error, Manifest};
    use std::collections::BTreeMap;

    #[test]
    fn duplicate_output_is_an_error() {
        let mut build = Build {
            manifest: Manifest::empty(),
            files: BTreeMap::new(),
        };
        build
            .insert("client/app.js".into(), b"one".to_vec(), Some("/assets"))
            .unwrap();
        build
            .insert("client/app.js".into(), b"one".to_vec(), Some("/assets"))
            .unwrap();
        assert!(matches!(
            build.insert("client/app.js".into(), b"two".to_vec(), Some("/assets")),
            Err(Error::DuplicateOutput(_))
        ));
    }
}
