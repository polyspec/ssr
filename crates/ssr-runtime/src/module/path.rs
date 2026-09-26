use crate::Error;
use std::collections::BTreeMap;

#[derive(Clone)]
pub(crate) struct Sources {
    pub(super) entry: String,
    pub(super) files: BTreeMap<String, String>,
}

impl Sources {
    pub(crate) fn new(
        entry: (String, Vec<u8>),
        chunks: Vec<(String, Vec<u8>)>,
    ) -> Result<Self, Error> {
        let entry_path = entry.0.clone();
        let mut files = BTreeMap::new();
        for (path, bytes) in std::iter::once(entry).chain(chunks) {
            if !valid_path(&path) {
                return Err(Error::InvalidBundle("invalid server JavaScript path"));
            }
            let source = String::from_utf8(bytes)
                .map_err(|_| Error::InvalidBundle("UTF-8 server JavaScript required"))?;
            if source.is_empty() {
                return Err(Error::InvalidBundle("server JavaScript is empty"));
            }
            if files.insert(path, source).is_some() {
                return Err(Error::InvalidBundle("duplicate server JavaScript path"));
            }
        }
        Ok(Self {
            entry: entry_path,
            files,
        })
    }

    pub(crate) fn cache_bytes(&self) -> Vec<u8> {
        let mut bytes = b"server-modules\0".to_vec();
        bytes.extend_from_slice(&(self.entry.len() as u64).to_le_bytes());
        bytes.extend_from_slice(self.entry.as_bytes());
        for (path, source) in &self.files {
            bytes.extend_from_slice(&(path.len() as u64).to_le_bytes());
            bytes.extend_from_slice(path.as_bytes());
            bytes.extend_from_slice(&(source.len() as u64).to_le_bytes());
            bytes.extend_from_slice(source.as_bytes());
        }
        bytes
    }
}

fn valid_path(path: &str) -> bool {
    path.starts_with("server/")
        && path.ends_with(".js")
        && path.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && !part.contains('\\')
                && !part.contains(':')
                && !part.chars().any(char::is_control)
        })
}

pub(super) fn resolve(referrer: &str, specifier: &str) -> Option<String> {
    if !specifier.starts_with("./") && !specifier.starts_with("../") {
        return None;
    }
    if specifier.contains(&['\\', ':', '?', '#'][..]) || specifier.chars().any(char::is_control) {
        return None;
    }
    let mut parts = referrer.split('/').collect::<Vec<_>>();
    parts.pop()?;
    for part in specifier.split('/') {
        match part {
            "" => return None,
            "." => {}
            ".." => {
                if parts.len() <= 1 {
                    return None;
                }
                parts.pop();
            }
            part => parts.push(part),
        }
    }
    let result = parts.join("/");
    valid_path(&result).then_some(result)
}
