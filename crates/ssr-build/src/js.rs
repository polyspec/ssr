use std::path::Path;

use rolldown::plugin::Plugin;
use rolldown::{
    AssetFilenamesOutputOption, Bundler, BundlerOptions, ChunkFilenamesOutputOption,
    CodeSplittingMode, InputItem, ModuleType, OutputFormat, Platform,
};
use rolldown_common::Output;

use crate::{Build, BuildConfig, Error, asset_url::AssetUrlPlugin};

pub(crate) async fn bundle(
    config: &BuildConfig,
    entry: &Path,
    name: &str,
    result: &mut Build,
) -> Result<(), Error> {
    let types = [
        "png", "svg", "jpg", "gif", "webp", "avif", "ico", "woff", "woff2", "ttf",
    ]
    .map(|extension| (format!(".{extension}"), ModuleType::Asset))
    .into_iter()
    .collect();
    let entry = entry.to_str().ok_or_else(|| {
        Error::InvalidInput(format!("entry path is not UTF-8: {}", entry.display()))
    })?;
    let options = BundlerOptions {
        cwd: Some(config.root.clone()),
        input: Some(vec![InputItem {
            import: entry.into(),
            name: Some(name.into()),
        }]),
        platform: Some(Platform::Browser),
        format: Some(OutputFormat::Esm),
        code_splitting: Some(CodeSplittingMode::Bool(true)),
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
    let mut bundler = Bundler::with_plugins(
        options,
        vec![AssetUrlPlugin::new_shared(AssetUrlPlugin::new(
            config.root.clone(),
            config.asset_route.clone(),
        ))],
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
        let directory = if public { "client" } else { "server" };
        let path = format!("{directory}/{filename}");
        let artifact = result.insert(
            path,
            asset.content_as_bytes().to_vec(),
            public.then_some(config.asset_route.as_str()),
        )?;
        if is_entry {
            entry_found = true;
            if public {
                result.manifest.client = artifact;
            } else {
                result.manifest.server = artifact;
            }
        } else if public {
            result.manifest.assets.push(artifact);
        } else if filename.ends_with(".js") {
            result.manifest.server_chunks.push(artifact);
        }
    }
    if !entry_found {
        return Err(Error::JavaScript(format!("{name} entry was not emitted")));
    }
    Ok(())
}
