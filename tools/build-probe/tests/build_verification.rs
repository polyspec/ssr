#[path = "../../../crates/polyspec-ssr-build/tests/fixture/mod.rs"]
mod fixture;
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use lightningcss::bundler::{Bundler as CssBundler, FileProvider, ResolveResult, SourceProvider};
use lightningcss::dependencies::{Dependency, DependencyOptions};
use lightningcss::printer::PrinterOptions;
use lightningcss::stylesheet::ParserOptions;
use rolldown::{
    AssetFilenamesOutputOption, Bundler, BundlerOptions, ChunkFilenamesOutputOption,
    CodeSplittingMode, InputItem, ModuleType, OutputFormat, Platform, ResolveOptions,
};
use sha2::{Digest, Sha256};

fn asset_name(path: &Path, bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let hash = digest[..8]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let stem = path.file_stem().unwrap().to_str().unwrap();
    let extension = path.extension().unwrap().to_str().unwrap();
    format!("assets/{stem}-{hash}.{extension}")
}

struct PackageProvider {
    files: FileProvider,
    packages: PathBuf,
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
            originating_file.parent().unwrap().join(specifier)
        } else {
            self.packages.join(specifier)
        };
        Ok(ResolveResult::File(path.canonicalize()?))
    }
}

fn css_build(root: &Path, packages: &Path) -> (String, BTreeMap<String, Vec<u8>>) {
    let provider = PackageProvider {
        files: FileProvider::new(),
        packages: packages.to_path_buf(),
    };
    let mut bundler = CssBundler::new(&provider, None, ParserOptions::default());
    let stylesheet = bundler.bundle(&root.join("app.css")).unwrap();
    let result = stylesheet
        .to_css(PrinterOptions {
            analyze_dependencies: Some(DependencyOptions {
                remove_imports: true,
            }),
            ..Default::default()
        })
        .unwrap();
    let mut css = result.code;
    let mut assets = BTreeMap::new();
    for dependency in result.dependencies.unwrap() {
        match dependency {
            Dependency::Url(url) => {
                assert!(!url.url.starts_with("data:"));
                let path = Path::new(&url.loc.file_path)
                    .parent()
                    .unwrap()
                    .join(&url.url);
                let bytes = fs::read(&path).unwrap();
                let name = asset_name(&path, &bytes);
                assert!(css.contains(&url.placeholder));
                css = css.replace(&url.placeholder, &format!("/{name}"));
                assets.insert(name, bytes);
            }
            Dependency::Import(import) => panic!("unbundled CSS import: {}", import.url),
        }
    }
    assert!(!css.contains("@import"));
    (css, assets)
}

async fn js_build(root: &Path, packages: &Path, entry: &str) -> BTreeMap<String, Vec<u8>> {
    let types = [
        "png", "svg", "jpg", "gif", "webp", "avif", "ico", "woff", "woff2", "ttf",
    ]
    .map(|extension| (format!(".{extension}"), ModuleType::Asset))
    .into_iter()
    .collect();
    let mut bundler = Bundler::new(BundlerOptions {
        cwd: Some(root.to_path_buf()),
        input: Some(vec![InputItem {
            import: format!("./{entry}.tsx"),
            name: Some(entry.into()),
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
        resolve: Some(ResolveOptions {
            modules: Some(vec![packages.to_string_lossy().into_owned()]),
            ..Default::default()
        }),
        ..Default::default()
    })
    .unwrap();
    let output = bundler.generate().await.unwrap();
    assert!(
        output.warnings.is_empty(),
        "warnings: {:?}",
        output.warnings
    );
    output
        .assets
        .into_iter()
        .map(|asset| {
            (
                asset.filename().to_owned(),
                asset.content_as_bytes().to_vec(),
            )
        })
        .collect()
}

#[tokio::test]
async fn react_tsx_css_and_assets_build_with_rust_apis() {
    let fixture = fixture::Fixture::new();
    let root = fixture.root.clone();
    let server = js_build(&root, &fixture.packages, "server").await;
    let client = js_build(&root, &fixture.packages, "client").await;
    for (name, output, marker) in [
        ("server", &server, "renderToString"),
        ("client", &client, "createRoot"),
    ] {
        let entry = output
            .iter()
            .find(|(path, _)| path.starts_with(&format!("{name}-")) && path.ends_with(".js"))
            .unwrap();
        assert!(entry.0.len() >= name.len() + 12, "{}", entry.0);
        assert!(String::from_utf8_lossy(entry.1).contains(marker));
        assert!(output.iter().any(|(path, bytes)| path.starts_with("extra-")
            && String::from_utf8_lossy(bytes).contains("split chunk content")));
        for extension in [
            "png", "svg", "jpg", "gif", "webp", "avif", "ico", "woff", "woff2", "ttf",
        ] {
            let source = root.join(format!("assets/sample.{extension}"));
            let bytes = fs::read(&source).unwrap();
            assert!(
                output
                    .iter()
                    .any(|(path, emitted)| path.starts_with("assets/sample-")
                        && path.ends_with(&format!(".{extension}"))
                        && *emitted == bytes),
                "missing {extension} in {name}"
            );
        }
    }
    let (css, css_assets) = css_build(&root, &fixture.packages);
    assert!(css.contains("font-family: Inter"));
    assert!(css.contains("/assets/sample-"));
    assert!(css_assets.keys().any(|name| name.ends_with(".svg")));
    assert!(css_assets.keys().any(|name| name.ends_with(".woff2")));
    for asset in css_assets.keys() {
        assert!(css.contains(&format!("/{asset}")));
    }
}

#[tokio::test]
async fn missing_js_asset_and_css_import_fail() {
    let fixture = fixture::Fixture::new();
    let root = fixture.root.clone();
    let mut js = Bundler::new(BundlerOptions {
        cwd: Some(root.clone()),
        input: Some(vec![InputItem {
            import: "./missing.tsx".into(),
            ..Default::default()
        }]),
        module_types: Some([(".png".into(), ModuleType::Asset)].into_iter().collect()),
        ..Default::default()
    })
    .unwrap();
    assert!(js.generate().await.is_err());

    let provider = PackageProvider {
        files: FileProvider::new(),
        packages: fixture.packages.clone(),
    };
    let mut css = CssBundler::new(&provider, None, ParserOptions::default());
    assert!(css.bundle(&root.join("missing.css")).is_err());
}
