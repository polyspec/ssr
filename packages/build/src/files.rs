use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use crate::{Build, Error, digest, manifest_read};

fn invalid(message: impl Into<String>) -> Error {
    Error::InvalidInput(message.into())
}

fn directory(path: &Path) -> Result<(), Error> {
    if !path.is_absolute() || !fs::symlink_metadata(path)?.is_dir() || path.canonicalize()? != path
    {
        return Err(invalid(format!(
            "build directory must be absolute without symbolic links: {}",
            path.display()
        )));
    }
    Ok(())
}

fn regular(path: &Path) -> Result<(), Error> {
    if !fs::symlink_metadata(path)?.file_type().is_file() {
        return Err(invalid(format!(
            "build path is not a regular file: {}",
            path.display()
        )));
    }
    Ok(())
}

fn scan(path: &Path, expected: &BTreeSet<PathBuf>) -> Result<(), Error> {
    let mut pending = vec![path.to_path_buf()];
    let mut actual = BTreeSet::new();
    while let Some(parent) = pending.pop() {
        for entry in fs::read_dir(parent)? {
            let entry = entry?;
            let target = entry.path();
            let kind = entry.file_type()?;
            if !expected.contains(&target) || (!kind.is_file() && !kind.is_dir()) {
                return Err(invalid(format!(
                    "unexpected build path: {}",
                    target.display()
                )));
            }
            if kind.is_dir() {
                pending.push(target.clone());
            }
            actual.insert(target);
        }
    }
    if actual != *expected {
        return Err(invalid("build files are missing"));
    }
    Ok(())
}

impl Build {
    pub fn read(path: &Path) -> Result<Self, Error> {
        directory(path)?;
        let manifest_path = path.join("manifest.json");
        regular(&manifest_path)?;
        let source = fs::read(&manifest_path)?;
        let manifest = manifest_read::parse(&source)?;
        manifest_read::validate_manifest(&manifest)?;
        let mut expected = BTreeSet::from([manifest_path]);
        for file in manifest_read::entries(&manifest) {
            let file_path = path.join(&file.path);
            expected.insert(file_path.clone());
            for parent in file_path
                .ancestors()
                .skip(1)
                .take_while(|parent| *parent != path)
            {
                expected.insert(parent.to_path_buf());
            }
        }
        scan(path, &expected)?;
        let mut files = BTreeMap::new();
        for file in manifest_read::entries(&manifest) {
            let target = path.join(&file.path);
            regular(&target)?;
            files.insert(file.path.clone(), fs::read(target)?);
        }
        let build = Self { manifest, files };
        let bytes = manifest_read::validate(&build)?;
        if source != bytes {
            return Err(invalid("build manifest bytes are not canonical"));
        }
        if path.file_name().and_then(|name| name.to_str()) != Some(digest(&bytes).as_str()) {
            return Err(invalid(
                "build directory name differs from manifest SHA-256",
            ));
        }
        Ok(build)
    }

    pub fn write(&self, path: &Path) -> Result<PathBuf, Error> {
        directory(path)?;
        let manifest = manifest_read::validate(self)?;
        let target = path.join(digest(&manifest));
        match fs::symlink_metadata(&target) {
            Ok(_) => return self.verify(&target).map(|()| target),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        let mut random = [0u8; 16];
        getrandom::fill(&mut random).map_err(Error::Random)?;
        let temporary = path.join(format!(".build-{}", digest(&random)));
        fs::DirBuilder::new().mode(0o755).create(&temporary)?;
        let result = self.publish(&temporary, &target, &manifest);
        let cleanup = match fs::remove_dir_all(&temporary) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            result => result,
        };
        match (result, cleanup) {
            (Err(operation), Err(cleanup)) => Err(Error::Cleanup {
                operation: Box::new(operation),
                cleanup,
            }),
            (Err(error), Ok(())) => Err(error),
            (Ok(()), Err(error)) => Err(error.into()),
            (Ok(()), Ok(())) => Ok(target),
        }
    }

    fn verify(&self, path: &Path) -> Result<(), Error> {
        let current = Self::read(path)?;
        if current.files != self.files
            || manifest_read::validate(&current)? != manifest_read::validate(self)?
        {
            return Err(invalid(format!(
                "build directory contains different content: {}",
                path.display()
            )));
        }
        Ok(())
    }

    fn publish(&self, temporary: &Path, target: &Path, manifest: &[u8]) -> Result<(), Error> {
        let mut directories = BTreeSet::from([temporary.to_path_buf()]);
        for (name, bytes) in self
            .files
            .iter()
            .map(|(name, bytes)| (name.as_str(), bytes.as_slice()))
            .chain(std::iter::once(("manifest.json", manifest)))
        {
            let output = temporary.join(name);
            let parent = output
                .parent()
                .ok_or_else(|| invalid("build file has no parent"))?;
            fs::create_dir_all(parent)?;
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o644)
                .open(&output)?;
            file.write_all(bytes)?;
            file.sync_all()?;
            for parent in output
                .ancestors()
                .skip(1)
                .take_while(|parent| parent.starts_with(temporary))
            {
                directories.insert(parent.to_path_buf());
            }
        }
        for path in directories.iter().rev() {
            File::open(path)?.sync_all()?;
        }
        match fs::rename(temporary, target) {
            Ok(()) => {}
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::AlreadyExists | io::ErrorKind::DirectoryNotEmpty
                ) =>
            {
                self.verify(target)?
            }
            Err(error) => return Err(error.into()),
        }
        File::open(
            target
                .parent()
                .ok_or_else(|| invalid("build directory has no parent"))?,
        )?
        .sync_all()?;
        Ok(())
    }
}
