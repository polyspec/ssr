use polyspec_ssr_build::{BuildConfig, build};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "ssr-exports-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        Self(root.canonicalize().unwrap())
    }
    fn config(&self, import: &str) -> BuildConfig {
        fs::write(self.0.join("entry.js"), format!("import {{ marker }} from '{import}'; export function render(props,state) {{return {{head:'',html:marker,state}};}}")).unwrap();
        fs::write(self.0.join("client.js"), "console.log('client');").unwrap();
        fs::write(self.0.join("app.css"), "main{color:red}").unwrap();
        BuildConfig {
            root: self.0.clone(),
            server_entry: self.0.join("entry.js"),
            react_framework_entry: None,
            client_entry: self.0.join("client.js"),
            css_entry: self.0.join("app.css"),
            asset_route: "/assets".into(),
            dependencies: Some(self.0.join("dependencies")),
        }
    }
    fn install(&self) {
        let package = self.0.join("dependencies/@scope/value");
        fs::create_dir_all(package.join("dist")).unwrap();
        fs::write(package.join("package.json"), r#"{"name":"@scope/value","main":"dist/index.js","exports":{".":"./dist/index.js","./internal":{"import":"./dist/internal.mjs","require":"./dist/internal.cjs"}}}"#).unwrap();
        fs::write(
            package.join("dist/index.js"),
            "export const marker='root-export';",
        )
        .unwrap();
        fs::write(
            package.join("dist/internal.mjs"),
            "export const marker='import-export';",
        )
        .unwrap();
        fs::write(
            package.join("dist/internal.cjs"),
            "exports.marker='require-export';",
        )
        .unwrap();
        fs::write(
            package.join("hidden.js"),
            "export const marker='hidden-file';",
        )
        .unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("fixture cleanup");
    }
}

#[tokio::test]
async fn configured_dependencies_resolve_scoped_conditional_exports() {
    let fixture = Fixture::new();
    fixture.install();
    let output = build(&fixture.config("@scope/value/internal"))
        .await
        .expect("exported subpath");
    let server = String::from_utf8(output.files[&output.manifest.server.path].clone()).unwrap();
    assert!(server.contains("import-export"));
    assert!(!server.contains("require-export"));
}

#[tokio::test]
async fn configured_dependencies_reject_unexported_existing_files() {
    let fixture = Fixture::new();
    fixture.install();
    assert!(
        build(&fixture.config("@scope/value/hidden")).await.is_err(),
        "a file outside exports must not be imported"
    );
}
