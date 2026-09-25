use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use lightningcss::bundler::{Bundler, FileProvider, ResolveResult, SourceProvider};
use lightningcss::dependencies::{Dependency, DependencyOptions};
use lightningcss::printer::PrinterOptions;
use lightningcss::stylesheet::ParserOptions;

use crate::{Build, BuildConfig, BuildFile, Error, digest};

struct PackageProvider {
    files: FileProvider,
    root: PathBuf,
}

impl SourceProvider for PackageProvider {
    type Error = io::Error;

    fn read<'a>(&'a self, file: &Path) -> Result<&'a str, Self::Error> {
        self.files.read(file)
    }

    fn resolve(
        &self,
        specifier: &str,
        originating_file: &Path,
    ) -> Result<ResolveResult, Self::Error> {
        let path = if specifier.starts_with("./") || specifier.starts_with("../") {
            originating_file
                .parent()
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "CSS source has no parent")
                })?
                .join(specifier)
        } else {
            self.root.join("node_modules").join(specifier)
        };
        Ok(ResolveResult::File(path.canonicalize()?))
    }
}

pub(crate) fn bundle(config: &BuildConfig, result: &mut Build) -> Result<BuildFile, Error> {
    let provider = PackageProvider {
        files: FileProvider::new(),
        root: config.root.clone(),
    };
    let mut bundler = Bundler::new(&provider, None, ParserOptions::default());
    let sheet = bundler
        .bundle(&config.css_entry)
        .map_err(|error| Error::Css(error.to_string()))?;
    let printed = sheet
        .to_css(PrinterOptions {
            analyze_dependencies: Some(DependencyOptions {
                remove_imports: true,
            }),
            ..Default::default()
        })
        .map_err(|error| Error::Css(error.to_string()))?;
    let mut code = printed.code;
    for dependency in printed
        .dependencies
        .ok_or_else(|| Error::Css("dependency analysis returned no result".into()))?
    {
        match dependency {
            Dependency::Import(import) => {
                return Err(Error::Css(format!("unbundled import: {}", import.url)));
            }
            Dependency::Url(url) => {
                if !url.url.starts_with("./") && !url.url.starts_with("../") {
                    return Err(Error::Css(format!("unsupported CSS URL: {}", url.url)));
                }
                let source = Path::new(&url.loc.file_path);
                let path = source
                    .parent()
                    .ok_or_else(|| Error::Css("CSS source has no parent".into()))?
                    .join(&url.url)
                    .canonicalize()?;
                if !path.starts_with(&config.root) || !path.is_file() {
                    return Err(Error::Css(format!(
                        "CSS asset outside application root: {}",
                        path.display()
                    )));
                }
                let bytes = fs::read(&path)?;
                let hash = digest(&bytes);
                let stem = path
                    .file_stem()
                    .and_then(|value| value.to_str())
                    .ok_or_else(|| Error::Css("CSS asset name is not UTF-8".into()))?;
                let extension = path
                    .extension()
                    .and_then(|value| value.to_str())
                    .ok_or_else(|| Error::Css("CSS asset extension is not UTF-8".into()))?;
                let name = format!("assets/{stem}-{}.{extension}", &hash[..16]);
                let artifact =
                    result.insert(format!("client/{name}"), bytes, Some(&config.asset_route))?;
                let replacement = artifact
                    .url
                    .as_deref()
                    .ok_or_else(|| Error::Css("CSS asset has no URL".into()))?;
                if !code.contains(&url.placeholder) {
                    return Err(Error::Css(format!(
                        "CSS URL placeholder missing: {}",
                        url.url
                    )));
                }
                code = code.replace(&url.placeholder, replacement);
                result.manifest.assets.push(artifact);
            }
        }
    }
    let hash = digest(code.as_bytes());
    result.insert(
        format!("client/styles-{}.css", &hash[..16]),
        code.into_bytes(),
        Some(&config.asset_route),
    )
}
