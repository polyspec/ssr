use ordered_json::{OrderedMap, Value};

use crate::{Error, digest, public_url};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildFile {
    pub path: String,
    pub url: Option<String>,
    pub sha256: String,
    pub content_type: String,
}

impl BuildFile {
    pub(crate) fn new(path: &str, bytes: &[u8], route: Option<&str>) -> Result<Self, Error> {
        let url = match route {
            Some(route) => {
                let public_path = path.strip_prefix("client/").ok_or_else(|| {
                    Error::InvalidInput(format!("public output is outside client: {path}"))
                })?;
                Some(public_url(route, public_path))
            }
            None => None,
        };
        let content_type = match path.rsplit('.').next() {
            Some("js") => "text/javascript; charset=utf-8",
            Some("map") => "application/json; charset=utf-8",
            Some("css") => "text/css; charset=utf-8",
            Some("png") => "image/png",
            Some("svg") => "image/svg+xml",
            Some("jpg") => "image/jpeg",
            Some("gif") => "image/gif",
            Some("webp") => "image/webp",
            Some("avif") => "image/avif",
            Some("ico") => "image/x-icon",
            Some("woff") => "font/woff",
            Some("woff2") => "font/woff2",
            Some("ttf") => "font/ttf",
            _ => {
                return Err(Error::InvalidInput(format!(
                    "unsupported output type: {path}"
                )));
            }
        };
        Ok(Self {
            path: path.into(),
            url,
            sha256: digest(bytes),
            content_type: content_type.into(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub server: BuildFile,
    pub react_framework: Option<BuildFile>,
    pub source_maps: Vec<BuildFile>,
    pub client: BuildFile,
    pub styles: Vec<BuildFile>,
    pub server_chunks: Vec<BuildFile>,
    pub assets: Vec<BuildFile>,
}

impl Manifest {
    pub(crate) fn empty() -> Self {
        let empty = BuildFile {
            path: String::new(),
            url: None,
            sha256: String::new(),
            content_type: String::new(),
        };
        Self {
            server: empty.clone(),
            react_framework: None,
            source_maps: Vec::new(),
            client: empty,
            styles: Vec::new(),
            server_chunks: Vec::new(),
            assets: Vec::new(),
        }
    }

    pub(crate) fn sort(&mut self) {
        self.source_maps.sort_by(|a, b| a.path.cmp(&b.path));
        self.styles.sort_by(|a, b| a.path.cmp(&b.path));
        self.server_chunks.sort_by(|a, b| a.path.cmp(&b.path));
        self.assets.sort_by(|a, b| a.path.cmp(&b.path));
        self.assets.dedup_by(|a, b| a.path == b.path);
    }

    pub fn to_json(&self) -> Result<Vec<u8>, Error> {
        fn artifact(value: &BuildFile) -> Result<Value, ordered_json::Error> {
            let mut fields = OrderedMap::new();
            fields.insert(Value::string("path"), Value::string(&value.path))?;
            fields.insert(
                Value::string("url"),
                value.url.as_deref().map_or_else(Value::null, Value::string),
            )?;
            fields.insert(Value::string("sha256"), Value::string(&value.sha256))?;
            fields.insert(
                Value::string("contentType"),
                Value::string(&value.content_type),
            )?;
            Value::object(&fields)
        }
        fn list(values: &[BuildFile]) -> Result<Value, ordered_json::Error> {
            Value::array(&values.iter().map(artifact).collect::<Result<Vec<_>, _>>()?)
        }
        let mut fields = OrderedMap::new();
        fields.insert(Value::string("server"), artifact(&self.server)?)?;
        fields.insert(
            Value::string("reactFramework"),
            match &self.react_framework {
                Some(file) => artifact(file)?,
                None => Value::null(),
            },
        )?;
        fields.insert(Value::string("sourceMaps"), list(&self.source_maps)?)?;
        fields.insert(Value::string("client"), artifact(&self.client)?)?;
        fields.insert(Value::string("styles"), list(&self.styles)?)?;
        fields.insert(Value::string("serverChunks"), list(&self.server_chunks)?)?;
        fields.insert(Value::string("assets"), list(&self.assets)?)?;
        Ok(Value::object(&fields)?.compact().into_bytes())
    }
}
