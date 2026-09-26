use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use crate::{Build, BuildFile, digest};

#[derive(Debug)]
pub enum PublishError {
    Invalid(String),
    Conflict(PathBuf),
    Io(io::Error),
    Random(getrandom::Error),
    Cleanup {
        operation: Box<Self>,
        cleanup: io::Error,
    },
}

impl std::fmt::Display for PublishError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(reason) => write!(f, "invalid public build file: {reason}"),
            Self::Conflict(path) => write!(
                f,
                "published file has different content: {}",
                path.display()
            ),
            Self::Io(error) => write!(f, "publication I/O failed: {error}"),
            Self::Random(error) => write!(f, "publication random value failed: {error}"),
            Self::Cleanup { operation, cleanup } => {
                write!(f, "{operation}; temporary file removal failed: {cleanup}")
            }
        }
    }
}

impl std::error::Error for PublishError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Random(error) => Some(error),
            Self::Cleanup { operation, .. } => Some(operation),
            _ => None,
        }
    }
}

impl From<io::Error> for PublishError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

#[derive(Debug, Clone)]
pub struct PublicFile {
    bytes: Vec<u8>,
    content_type: String,
    sha256: String,
}

impl PublicFile {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn content_type(&self) -> &str {
        &self.content_type
    }

    pub fn sha256(&self) -> &str {
        &self.sha256
    }
}

#[derive(Debug, Clone)]
pub struct PublicFiles {
    files: BTreeMap<String, PublicFile>,
}

impl PublicFiles {
    pub fn new(build: &Build) -> Result<Self, PublishError> {
        if build.manifest.server.url.is_some()
            || build
                .manifest
                .server_chunks
                .iter()
                .any(|file| file.url.is_some())
        {
            return Err(PublishError::Invalid(
                "server output has a public URL".into(),
            ));
        }
        let mut files = BTreeMap::new();
        for file in std::iter::once(&build.manifest.client)
            .chain(build.manifest.styles.iter())
            .chain(build.manifest.assets.iter())
        {
            let url = file.url.as_ref().ok_or_else(|| {
                PublishError::Invalid(format!("missing public URL: {}", file.path))
            })?;
            validate_url(file, url)?;
            let bytes = build
                .files
                .get(&file.path)
                .ok_or_else(|| PublishError::Invalid(format!("missing output: {}", file.path)))?;
            if digest(bytes) != file.sha256 {
                return Err(PublishError::Invalid(format!(
                    "SHA-256 mismatch: {}",
                    file.path
                )));
            }
            let expected = BuildFile::new(&file.path, bytes, None)
                .map_err(|error| PublishError::Invalid(error.to_string()))?;
            if expected.content_type != file.content_type {
                return Err(PublishError::Invalid(format!(
                    "content type mismatch: {}",
                    file.path
                )));
            }
            if files
                .insert(
                    url.clone(),
                    PublicFile {
                        bytes: bytes.clone(),
                        content_type: file.content_type.clone(),
                        sha256: file.sha256.clone(),
                    },
                )
                .is_some()
            {
                return Err(PublishError::Invalid(format!(
                    "duplicate public URL: {url}"
                )));
            }
        }
        Ok(Self { files })
    }

    pub fn get(&self, url: &str) -> Option<&PublicFile> {
        self.files.get(url)
    }

    pub fn publish(&self, directory: &Path) -> Result<(), PublishError> {
        if !directory.is_absolute() || !directory.is_dir() || directory.canonicalize()? != directory
        {
            return Err(PublishError::Invalid(format!(
                "publication directory must be an existing absolute directory without symbolic links: {}",
                directory.display()
            )));
        }
        for (url, file) in &self.files {
            let target = directory.join(url.trim_start_matches('/'));
            ensure_parent(directory, url, false)?;
            match fs::symlink_metadata(&target) {
                Ok(metadata) if !metadata.file_type().is_file() => {
                    return Err(PublishError::Invalid(format!(
                        "target is not a regular file: {}",
                        target.display()
                    )));
                }
                Ok(_) if fs::read(&target)? != file.bytes => {
                    return Err(PublishError::Conflict(target));
                }
                Ok(_) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        for (url, file) in &self.files {
            let target = directory.join(url.trim_start_matches('/'));
            ensure_parent(directory, url, true)?;
            publish_file(&target, &file.bytes)?;
        }
        Ok(())
    }
}

fn ensure_parent(directory: &Path, url: &str, create: bool) -> Result<(), PublishError> {
    let relative = Path::new(url.trim_start_matches('/'));
    let parent = relative
        .parent()
        .ok_or_else(|| PublishError::Invalid(format!("URL has no parent: {url}")))?;
    let mut current = directory.to_path_buf();
    for component in parent.components() {
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_dir() => {}
            Ok(_) => {
                return Err(PublishError::Invalid(format!(
                    "publication path is not a directory: {}",
                    current.display()
                )));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound && create => {
                match fs::create_dir(&current) {
                    Ok(()) => {}
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                    Err(error) => return Err(error.into()),
                }
                if !fs::symlink_metadata(&current)?.file_type().is_dir() {
                    return Err(PublishError::Invalid(format!(
                        "publication path is not a directory: {}",
                        current.display()
                    )));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn validate_url(file: &BuildFile, url: &str) -> Result<(), PublishError> {
    let public_path = file.path.strip_prefix("client/").ok_or_else(|| {
        PublishError::Invalid(format!("public output is outside client: {}", file.path))
    })?;
    if !url.starts_with('/')
        || url.starts_with("//")
        || !url.ends_with(public_path)
        || !url
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'_' | b'.'))
        || url
            .split('/')
            .skip(1)
            .any(|part| part.is_empty() || part == "." || part == "..")
        || file
            .path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(PublishError::Invalid(format!("invalid public URL: {url}")));
    }
    let prefix = url
        .strip_suffix(public_path)
        .ok_or_else(|| PublishError::Invalid(format!("URL does not match output: {url}")))?;
    if !prefix.ends_with('/') {
        return Err(PublishError::Invalid(format!(
            "URL does not match output: {url}"
        )));
    }
    Ok(())
}

fn publish_file(target: &Path, bytes: &[u8]) -> Result<(), PublishError> {
    match fs::symlink_metadata(target) {
        Ok(metadata) if !metadata.file_type().is_file() => {
            return Err(PublishError::Invalid(format!(
                "target is not a regular file: {}",
                target.display()
            )));
        }
        Ok(_) if fs::read(target)? != bytes => {
            return Err(PublishError::Conflict(target.to_path_buf()));
        }
        Ok(_) => return Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let parent = target.parent().ok_or_else(|| {
        PublishError::Invalid(format!("target has no parent: {}", target.display()))
    })?;
    let mut random = [0u8; 16];
    getrandom::fill(&mut random).map_err(PublishError::Random)?;
    let suffix = random
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let temporary = parent.join(format!(".publish-{suffix}"));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o644)
        .open(&temporary)?;
    let result = (|| -> Result<(), PublishError> {
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        match fs::hard_link(&temporary, target) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                match fs::symlink_metadata(target) {
                    Ok(metadata)
                        if metadata.file_type().is_file() && fs::read(target)? == bytes =>
                    {
                        Ok(())
                    }
                    Ok(metadata) if !metadata.file_type().is_file() => Err(PublishError::Invalid(
                        format!("target is not a regular file: {}", target.display()),
                    )),
                    Ok(_) => Err(PublishError::Conflict(target.to_path_buf())),
                    Err(error) => Err(error.into()),
                }
            }
            Err(error) => Err(error.into()),
        }
    })();
    let cleanup = fs::remove_file(&temporary);
    match (result, cleanup) {
        (Err(operation), Err(cleanup)) => Err(PublishError::Cleanup {
            operation: Box::new(operation),
            cleanup,
        }),
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Err(error)) => Err(error.into()),
        (Ok(()), Ok(())) => Ok(()),
    }
}
