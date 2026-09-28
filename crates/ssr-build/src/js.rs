use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rolldown::plugin::Plugin;
use rolldown::{
    AssetFilenamesOutputOption, Bundler, BundlerOptions, ChunkFilenamesOutputOption,
    CodeSplittingMode, InputItem, IsExternal, ModuleType, OutputFormat, Platform,
};
use rolldown_common::{GlobalsOutputOption, Output, ResolveOptions, SourceMapType};
use sourcemap::{SourceMap, SourceMapBuilder};

use crate::{
    Build, BuildConfig, Error, asset_url::AssetUrlPlugin, react_css::ReactCssPlugin,
    svelte::SveltePlugin,
};

/// Resolves every bare application package import from the configured
/// dependency directory only; a package under a nested `node_modules`
/// directory of the application source is never used.
#[derive(Debug)]
struct DependencyPlugin {
    packages: PathBuf,
    external: Vec<String>,
}

impl Plugin for DependencyPlugin {
    fn name(&self) -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("ssr-build-dependencies")
    }

    fn register_hook_usage(&self) -> rolldown::plugin::HookUsage {
        rolldown::plugin::HookUsage::ResolveId
    }

    async fn resolve_id(
        &self,
        _ctx: &rolldown::plugin::PluginContext,
        args: &rolldown::plugin::HookResolveIdArgs<'_>,
    ) -> rolldown::plugin::HookResolveIdReturn {
        let specifier = args.specifier;
        if specifier.starts_with('.')
            || specifier.starts_with('/')
            || specifier.starts_with('\0')
            || specifier.starts_with("node:")
            || specifier.contains(':')
            || self.external.iter().any(|name| name == specifier)
        {
            return Ok(None);
        }
        let name = specifier.split('/').next().unwrap_or(specifier);
        if !self.packages.join(name).symlink_metadata().is_ok() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!(
                    "application package {name:?} is not in the configured dependency directory {}",
                    self.packages.display()
                ),
            )
            .into());
        }
        match probe_package(&self.packages.join(specifier)) {
            Some(path) => Ok(Some(rolldown::plugin::HookResolveIdOutput::from_id(
                path.to_string_lossy().into_owned(),
            ))),
            None => Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!(
                    "application package import {specifier:?} has no entry in the configured dependency directory {}",
                    self.packages.display()
                ),
            )
            .into()),
        }
    }
}

/// Resolves a package path the way a package directory does: a file, a file
/// with a known extension, or a directory through its `package.json` entry or
/// index file.
fn probe_package(base: &Path) -> Option<PathBuf> {
    if base.is_file() {
        return Some(base.to_path_buf());
    }
    for extension in ["js", "jsx", "mjs", "cjs", "ts", "tsx", "json"] {
        let candidate = PathBuf::from(format!("{}.{}", base.display(), extension));
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    if base.is_dir() {
        let entry = std::fs::read_to_string(base.join("package.json"))
            .ok()
            .and_then(|manifest| {
                ordered_json::parse_bytes_reject_duplicates(manifest.as_bytes()).ok()
            })
            .and_then(|value| {
                value
                    .get("main")
                    .or_else(|| value.get("module"))
                    .and_then(|field| field.string_value().ok())
                    .filter(|entry| !entry.is_empty())
            });
        if let Some(entry) = entry
            && let Some(path) = probe_package(&base.join(entry.trim_start_matches("./")))
        {
            return Some(path);
        }
        for index in ["index.js", "index.mjs", "index.cjs"] {
            let candidate = base.join(index);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

pub(crate) async fn bundle(
    config: &BuildConfig,
    entry: &Path,
    name: &str,
    result: &mut Build,
) -> Result<BTreeMap<PathBuf, String>, Error> {
    let types = [
        "png", "svg", "jpg", "gif", "webp", "avif", "ico", "woff", "woff2", "ttf",
    ]
    .map(|extension| (format!(".{extension}"), ModuleType::Asset))
    .into_iter()
    .chain([(".svelte".to_owned(), ModuleType::Js)])
    .collect();
    let entry = entry.to_str().ok_or_else(|| {
        Error::InvalidInput(format!("entry path is not UTF-8: {}", entry.display()))
    })?;
    let react_application = name == "server" && config.react_framework_entry.is_some();
    let options = BundlerOptions {
        cwd: Some(config.root.clone()),
        input: Some(vec![InputItem {
            import: entry.into(),
            name: Some(name.into()),
        }]),
        platform: Some(Platform::Browser),
        resolve: Some(ResolveOptions {
            modules: Some(vec![
                config.package_directory().to_string_lossy().into_owned(),
            ]),
            ..Default::default()
        }),
        format: Some(if react_application {
            OutputFormat::Iife
        } else {
            OutputFormat::Esm
        }),
        code_splitting: Some(CodeSplittingMode::Bool(
            name == "client" || (name == "server" && !react_application),
        )),
        external: react_application
            .then(|| IsExternal::from(vec!["react".to_owned(), "react/jsx-runtime".to_owned()])),
        globals: react_application.then(|| {
            GlobalsOutputOption::FxHashMap(
                [
                    ("react".to_owned(), "__ssrReact".to_owned()),
                    ("react/jsx-runtime".to_owned(), "__ssrJsxRuntime".to_owned()),
                ]
                .into_iter()
                .collect(),
            )
        }),
        sourcemap: (name != "client").then_some(SourceMapType::Hidden),
        entry_filenames: Some(ChunkFilenamesOutputOption::String(
            "[name]-[hash].js".into(),
        )),
        chunk_filenames: Some(ChunkFilenamesOutputOption::String(
            "[name]-[hash].js".into(),
        )),
        asset_filenames: Some(AssetFilenamesOutputOption::String(
            "assets/[name]-[hash][extname]".into(),
        )),
        module_types: Some(types),
        ..Default::default()
    };
    let styles = Arc::new(Mutex::new(BTreeMap::new()));
    let external = if react_application {
        vec!["react".to_owned(), "react/jsx-runtime".to_owned()]
    } else {
        Vec::new()
    };
    let dependency_plugin = config.dependencies.as_ref().map(|packages| {
        DependencyPlugin::new_shared(DependencyPlugin {
            packages: packages.clone(),
            external,
        })
    });
    let mut bundler = Bundler::with_plugins(
        options,
        vec![
            AssetUrlPlugin::new_shared(AssetUrlPlugin::new(
                config.root.clone(),
                config.asset_route.clone(),
            )),
            SveltePlugin::new_shared(SveltePlugin::new(
                config.root.clone(),
                config.package_directory(),
                if name == "client" { "client" } else { "server" },
                Arc::clone(&styles),
            )),
            ReactCssPlugin::new_shared(ReactCssPlugin),
        ]
        .into_iter()
        .chain(dependency_plugin)
        .collect(),
    )
    .map_err(|error| Error::JavaScript(error.to_string()))?;
    let output = bundler
        .generate()
        .await
        .map_err(|error| Error::JavaScript(error.to_string()))?;
    if !output.warnings.is_empty() {
        return Err(Error::JavaScript(format!(
            "warnings: {:?}",
            output.warnings
        )));
    }
    let mut entry_found = false;
    for asset in output.assets {
        let filename = asset.filename();
        if Path::new(filename).is_absolute()
            || filename
                .split('/')
                .any(|part| part.is_empty() || part == "." || part == "..")
        {
            return Err(Error::JavaScript(format!(
                "invalid output path: {filename}"
            )));
        }
        let is_entry =
            matches!(&asset, Output::Chunk(chunk) if chunk.is_entry && chunk.name.as_str() == name);
        if is_entry && entry_found {
            return Err(Error::JavaScript(format!("multiple {name} entries")));
        }
        let public = name == "client" || !filename.ends_with(".js");
        let is_source_map = name != "client" && filename.ends_with(".js.map");
        let public = public && !is_source_map;
        let directory = if public { "client" } else { "server" };
        let path = format!("{directory}/{filename}");
        let bytes = if is_source_map {
            canonical_source_map(asset.content_as_bytes())?
        } else {
            asset.content_as_bytes().to_vec()
        };
        let artifact = result.insert(path, bytes, public.then_some(config.asset_route.as_str()))?;
        if is_entry {
            entry_found = true;
            if public {
                result.manifest.client = artifact;
            } else if name == "react_framework" {
                result.manifest.react_framework = Some(artifact);
            } else {
                result.manifest.server = artifact;
            }
        } else if public {
            result.manifest.assets.push(artifact);
        } else if is_source_map {
            result.manifest.source_maps.push(artifact);
        } else if filename.ends_with(".js") {
            result.manifest.server_chunks.push(artifact);
        }
    }
    if !entry_found {
        return Err(Error::JavaScript(format!("{name} entry was not emitted")));
    }
    styles
        .lock()
        .map_err(|_| Error::JavaScript("Svelte style lock failed".into()))
        .map(|styles| styles.clone())
}

fn canonical_source_map(bytes: &[u8]) -> Result<Vec<u8>, Error> {
    let map = SourceMap::from_slice(bytes).map_err(|error| Error::JavaScript(error.to_string()))?;
    let mut builder = SourceMapBuilder::new(map.get_file());
    builder.set_source_root(map.get_source_root());
    builder.set_debug_id(map.get_debug_id());
    let mut sources = (0..map.get_source_count())
        .map(|index| {
            map.get_source(index)
                .map(|name| (name, index))
                .ok_or_else(|| Error::JavaScript(format!("source map omits source {index}")))
        })
        .collect::<Result<Vec<_>, _>>()?;
    sources.sort_by(|left, right| left.0.cmp(right.0));
    for (name, old_index) in sources {
        let new_index = builder.add_source(name);
        if map.ignore_list().any(|index| *index == old_index) {
            builder.add_to_ignore_list(new_index);
        }
    }
    for token in map.tokens() {
        builder.add(
            token.get_dst_line(),
            token.get_dst_col(),
            token.get_src_line(),
            token.get_src_col(),
            token.get_source(),
            token.get_name(),
            token.is_range(),
        );
    }
    let mut output = Vec::new();
    builder
        .into_sourcemap()
        .to_writer(&mut output)
        .map_err(|error| Error::JavaScript(error.to_string()))?;
    Ok(output)
}
