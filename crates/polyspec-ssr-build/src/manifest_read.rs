use std::collections::BTreeSet;

use ordered_json::{Kind, Value};

use crate::{Build, BuildFile, Error, Manifest, PublicFiles};

fn invalid(message: impl Into<String>) -> Error {
    Error::InvalidInput(message.into())
}

fn field<'a>(value: &'a Value, name: &str) -> Result<&'a Value, Error> {
    value
        .get(name)
        .ok_or_else(|| invalid(format!("missing manifest field: {name}")))
}

fn object(value: &Value, count: usize) -> Result<(), Error> {
    match value.members() {
        Some(members) if members.len() == count => Ok(()),
        _ => Err(invalid("invalid manifest fields")),
    }
}

fn file(value: &Value) -> Result<BuildFile, Error> {
    object(value, 4)?;
    let url = field(value, "url")?;
    Ok(BuildFile {
        path: field(value, "path")?.string_value()?,
        url: if url.kind() == Kind::Null {
            None
        } else {
            Some(url.string_value()?)
        },
        sha256: field(value, "sha256")?.string_value()?,
        content_type: field(value, "contentType")?.string_value()?,
    })
}

fn list(value: &Value, name: &str) -> Result<Vec<BuildFile>, Error> {
    field(value, name)?
        .items()
        .ok_or_else(|| invalid(format!("manifest field is not an array: {name}")))?
        .iter()
        .map(file)
        .collect()
}

pub(crate) fn parse(bytes: &[u8]) -> Result<Manifest, Error> {
    let value = ordered_json::parse_bytes_reject_duplicates(bytes)?;
    object(&value, 7)?;
    let framework = field(&value, "reactFramework")?;
    Ok(Manifest {
        server: file(field(&value, "server")?)?,
        react_framework: if framework.kind() == Kind::Null {
            None
        } else {
            Some(file(framework)?)
        },
        source_maps: list(&value, "sourceMaps")?,
        client: file(field(&value, "client")?)?,
        styles: list(&value, "styles")?,
        server_chunks: list(&value, "serverChunks")?,
        assets: list(&value, "assets")?,
    })
}

pub(crate) fn entries(manifest: &Manifest) -> impl Iterator<Item = &BuildFile> {
    std::iter::once(&manifest.server)
        .chain(manifest.react_framework.iter())
        .chain(manifest.source_maps.iter())
        .chain(std::iter::once(&manifest.client))
        .chain(manifest.styles.iter())
        .chain(manifest.server_chunks.iter())
        .chain(manifest.assets.iter())
}

fn path(path: &str) -> Result<(), Error> {
    if !path
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'_' | b'.'))
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(invalid(format!("unsafe build path: {path}")));
    }
    Ok(())
}

fn private(file: &BuildFile, extension: &str) -> Result<(), Error> {
    if !file.path.starts_with("server/") || !file.path.ends_with(extension) || file.url.is_some() {
        return Err(invalid(format!(
            "invalid private build file: {}",
            file.path
        )));
    }
    Ok(())
}

pub(crate) fn validate_manifest(manifest: &Manifest) -> Result<(), Error> {
    let mut paths = BTreeSet::new();
    for file in entries(manifest) {
        path(&file.path)?;
        if !paths.insert(&file.path) {
            return Err(invalid(format!("duplicate build path: {}", file.path)));
        }
    }
    let mut maps = BTreeSet::new();
    for file in std::iter::once(&manifest.server)
        .chain(manifest.react_framework.iter())
        .chain(manifest.server_chunks.iter())
    {
        private(file, ".js")?;
        maps.insert(format!("{}.map", file.path));
    }
    for file in &manifest.source_maps {
        private(file, ".js.map")?;
    }
    if maps
        != manifest
            .source_maps
            .iter()
            .map(|file| file.path.clone())
            .collect()
    {
        return Err(invalid("server source maps do not match JavaScript files"));
    }
    if manifest.react_framework.is_some() && !manifest.server_chunks.is_empty() {
        return Err(invalid("React build contains server chunks"));
    }
    if !manifest.client.path.ends_with(".js")
        || manifest
            .styles
            .iter()
            .any(|file| !file.path.ends_with(".css"))
    {
        return Err(invalid("invalid client or style output"));
    }
    Ok(())
}

pub(crate) fn validate(build: &Build) -> Result<Vec<u8>, Error> {
    validate_manifest(&build.manifest)?;
    let entries: Vec<_> = entries(&build.manifest).collect();
    if entries.len() != build.files.len() {
        return Err(invalid("manifest does not list every build file"));
    }
    for file in entries {
        let bytes = build
            .files
            .get(&file.path)
            .ok_or_else(|| invalid(format!("missing build file: {}", file.path)))?;
        let expected = BuildFile::new(&file.path, bytes, None)?;
        if file.sha256 != expected.sha256 || file.content_type != expected.content_type {
            return Err(invalid(format!(
                "build digest or content type mismatch: {}",
                file.path
            )));
        }
    }
    PublicFiles::new(build).map_err(|error| invalid(error.to_string()))?;
    let mut manifest = build.manifest.clone();
    manifest.sort();
    manifest.to_json()
}
