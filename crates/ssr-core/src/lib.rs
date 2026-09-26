#![forbid(unsafe_code)]

pub use ordered_json::Value;
use ordered_json::{Kind, OrderedMap};
use std::fmt;

pub const CALL_PATH: &str = "/_render";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Render {
    Ssr,
    Csr,
}

impl Render {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ssr => "ssr",
            Self::Csr => "csr",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Page {
    pub render: Render,
    pub title: String,
    pub language: String,
    pub props: Value,
    pub state: Value,
}

#[derive(Debug, Clone)]
pub struct RenderResult {
    pub html: Vec<u8>,
    pub state: Value,
}

#[derive(Debug)]
pub enum Error {
    InvalidJson(ordered_json::Error),
    InvalidField(&'static str),
    Render(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson(error) => write!(f, "invalid render JSON: {error}"),
            Self::InvalidField(field) => write!(f, "invalid render field: {field}"),
            Self::Render(message) => write!(f, "render failed: {message}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidJson(error) => Some(error),
            Self::InvalidField(_) | Self::Render(_) => None,
        }
    }
}

impl From<ordered_json::Error> for Error {
    fn from(error: ordered_json::Error) -> Self {
        Self::InvalidJson(error)
    }
}

impl Page {
    pub fn from_json(source: &[u8]) -> Result<Self, Error> {
        let value = ordered_json::parse_bytes_reject_duplicates(source)?;
        let members = value.members().ok_or(Error::InvalidField("page"))?;
        let render = match required(&value, "render")?.string_value() {
            Ok(value) if value == "ssr" => Render::Ssr,
            Ok(value) if value == "csr" => Render::Csr,
            _ => return Err(Error::InvalidField("render")),
        };
        let title = required(&value, "title")?
            .string_value()
            .map_err(|_| Error::InvalidField("title"))?;
        let language = required(&value, "language")?
            .string_value()
            .map_err(|_| Error::InvalidField("language"))?;
        let props = required(&value, "props")?;
        if props.kind() != Kind::Object {
            return Err(Error::InvalidField("props"));
        }
        let state = required(&value, "state")?;
        if members.len() != 5 {
            return Err(Error::InvalidField("page fields"));
        }
        Ok(Self {
            render,
            title,
            language,
            props: props.clone(),
            state: state.clone(),
        })
    }

    pub fn to_json(&self) -> Result<Vec<u8>, Error> {
        if self.props.kind() != Kind::Object {
            return Err(Error::InvalidField("props"));
        }
        let mut members = OrderedMap::new();
        for (name, value) in [
            ("render", Value::string(self.render.as_str())),
            ("title", Value::string(&self.title)),
            ("language", Value::string(&self.language)),
            ("props", self.props.clone()),
            ("state", self.state.clone()),
        ] {
            members.insert(Value::string(name), value)?;
        }
        Ok(Value::object(&members)?.compact().into_bytes())
    }
}

fn required<'a>(value: &'a Value, name: &'static str) -> Result<&'a Value, Error> {
    value.get(name).ok_or(Error::InvalidField(name))
}

#[cfg(test)]
mod tests {
    #[test]
    fn core_has_no_local_crate_dependencies() {
        let manifest = include_str!("../Cargo.toml");
        for crate_name in [
            "ssr-build",
            "ssr-runtime",
            "ssr-adapter-react",
            "ssr-adapter-vue",
            "ssr-adapter-svelte",
            "ssr-adapter-vanilla",
            "ssr-server",
        ] {
            assert!(!manifest.contains(crate_name), "{crate_name}");
        }
    }
}
