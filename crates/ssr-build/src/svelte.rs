use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use deno_core::{JsRuntime, RuntimeOptions, v8};
use ordered_json::Value;
use rolldown::ModuleType;
use rolldown::plugin::{
    HookResolveIdArgs, HookResolveIdOutput, HookResolveIdReturn, HookTransformArgs,
    HookTransformOutput, HookTransformReturn, HookUsage, Plugin, PluginContext, PluginHookMeta,
    PluginOrder, SharedTransformPluginContext,
};
use rolldown_sourcemap::{OwnedSourceMap, SourceMap};

#[derive(Debug)]
pub(crate) struct SveltePlugin {
    root: PathBuf,
    packages: PathBuf,
    mode: &'static str,
    styles: Arc<Mutex<BTreeMap<PathBuf, String>>>,
}

impl SveltePlugin {
    pub(crate) fn new(
        root: PathBuf,
        packages: PathBuf,
        mode: &'static str,
        styles: Arc<Mutex<BTreeMap<PathBuf, String>>>,
    ) -> Self {
        Self {
            root,
            packages,
            mode,
            styles,
        }
    }
}

struct Compiled {
    code: String,
    map: SourceMap,
    css: Option<String>,
}

fn required_string(value: &Value, name: &'static str) -> Result<String, String> {
    value
        .get(name)
        .ok_or_else(|| format!("Svelte compiler omitted {name}"))?
        .string_value()
        .map_err(|error| format!("Svelte compiler returned invalid {name}: {error}"))
}

fn compile(compiler: &str, source: &str, path: &Path, mode: &str) -> Result<Compiled, String> {
    ssr_core::process::build(|| compile_in_runtime(compiler, source, path, mode))
        .map_err(str::to_owned)?
}

fn compile_in_runtime(
    compiler: &str,
    source: &str,
    path: &Path,
    mode: &str,
) -> Result<Compiled, String> {
    tokio::runtime::Handle::try_current()
        .map_err(|_| "Svelte compilation requires a Tokio runtime".to_owned())?;
    let path = path
        .to_str()
        .ok_or_else(|| "Svelte source path is not UTF-8".to_owned())?;
    let mut runtime = JsRuntime::new(RuntimeOptions::default());
    runtime
        .execute_script("compiler-environment.js", "globalThis.Deno = undefined;")
        .map_err(|error| format!("Svelte compiler environment failed: {error}"))?;
    runtime
        .execute_script("svelte-compiler.js", compiler.to_owned())
        .map_err(|error| format!("Svelte compiler initialization failed: {error}"))?;
    let script = format!(
        "(() => {{ const result = svelte.compile({}, {{filename: {}, generate: {}, css: 'external', dev: false}}); if (result.warnings.length) throw new Error(result.warnings.map(w => w.message).join('\\n')); return JSON.stringify({{code: result.js.code, map: result.js.map.toString(), css: result.css ? result.css.code : null}}); }})()",
        Value::string(source).compact(),
        Value::string(path).compact(),
        Value::string(mode).compact(),
    );
    let output = runtime
        .execute_script("compile-svelte.js", script)
        .map_err(|error| format!("Svelte compilation failed: {error}"))?;
    let result = {
        let context = runtime.main_context();
        v8::scope!(let scope, runtime.v8_isolate());
        let context = v8::Local::new(scope, context);
        let scope = &mut v8::ContextScope::new(scope, context);
        let value = v8::Local::new(scope, output);
        let text = value
            .to_string(scope)
            .ok_or_else(|| "Svelte compiler result is not a string".to_owned())?;
        text.to_rust_string_lossy(scope)
    };
    let value = ordered_json::parse_bytes_reject_duplicates(result.as_bytes())
        .map_err(|error| format!("Svelte compiler result is invalid JSON: {error}"))?;
    let code = required_string(&value, "code")?;
    let map = required_string(&value, "map")?;
    let map = OwnedSourceMap::from_json_string(&map)
        .map_err(|error| format!("Svelte compiler source map is invalid: {error}"))?
        .into_inner();
    let css = match value.get("css") {
        Some(css) if css.compact() == "null" => None,
        Some(css) => Some(
            css.string_value()
                .map_err(|error| format!("Svelte compiler CSS is invalid: {error}"))?,
        ),
        None => return Err("Svelte compiler omitted CSS result".into()),
    };
    Ok(Compiled { code, map, css })
}

impl Plugin for SveltePlugin {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("ssr-build-svelte")
    }

    fn register_hook_usage(&self) -> HookUsage {
        HookUsage::Transform | HookUsage::ResolveId
    }

    async fn resolve_id(
        &self,
        _ctx: &PluginContext,
        args: &HookResolveIdArgs<'_>,
    ) -> HookResolveIdReturn {
        if args.specifier != "node:async_hooks" {
            return Ok(None);
        }
        let source = self
            .packages
            .join("svelte/src/internal/server/render-context.js");
        if args.importer != source.to_str() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "node:async_hooks is only allowed in Svelte's render context",
            )
            .into());
        }
        Ok(Some(HookResolveIdOutput {
            id: args.specifier.into(),
            external: Some(true.into()),
            ..Default::default()
        }))
    }

    fn transform_meta(&self) -> Option<PluginHookMeta> {
        Some(PluginHookMeta {
            order: Some(PluginOrder::Pre),
        })
    }

    async fn transform(
        &self,
        _ctx: SharedTransformPluginContext,
        args: &HookTransformArgs<'_>,
    ) -> HookTransformReturn {
        if !args.id.ends_with(".svelte") {
            return Ok(None);
        }
        let path = Path::new(args.id);
        if !path.is_absolute() || !path.starts_with(&self.root) || path.canonicalize()? != path {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "Svelte source must be a file under application root: {}",
                    args.id
                ),
            )
            .into());
        }
        let compiler_path = self.packages.join("svelte/compiler/index.js");
        if compiler_path.canonicalize()? != compiler_path || !compiler_path.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Svelte compiler must be a file without symbolic links",
            )
            .into());
        }
        let compiler = fs::read_to_string(compiler_path)?;
        let compiled = compile(&compiler, args.code, path, self.mode)
            .map_err(|message| io::Error::new(io::ErrorKind::InvalidData, message))?;
        if let Some(css) = compiled.css {
            let mut styles = self
                .styles
                .lock()
                .map_err(|_| io::Error::other("Svelte style lock failed"))?;
            if styles.insert(path.to_path_buf(), css).is_some() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Svelte source compiled twice in one bundle",
                )
                .into());
            }
        }
        Ok(Some(HookTransformOutput {
            code: Some(compiled.code),
            map: compiled.map.into(),
            module_type: Some(ModuleType::Js),
            ..Default::default()
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::compile;
    use std::fs;

    #[tokio::test]
    async fn compiler_returns_server_client_and_style_outputs() {
        let fixture = crate::fixture::Fixture::new();
        let root = fixture.root.clone();
        let compiler =
            fs::read_to_string(fixture.packages.join("svelte/compiler/index.js")).unwrap();
        let source =
            "<script>let { name } = $props();</script><h1>{name}</h1><style>h1{color:red}</style>";
        let path = root.join("App.svelte");
        let server = compile(&compiler, source, &path, "server").unwrap();
        let client = compile(&compiler, source, &path, "client").unwrap();
        assert!(server.code.contains("h1"));
        assert!(client.code.contains("h1"));
        assert_eq!(server.css, client.css);
        assert!(client.css.unwrap().contains("color"));
        assert!(compile(&compiler, "<script>let = ;</script>", &path, "client").is_err());
        assert!(
            compile(
                &compiler,
                "<script>let value = $state(1); const copy = value;</script><p>{copy}</p>",
                &path,
                "client"
            )
            .is_err()
        );
        let application_path = root.join("SvelteApp.svelte");
        let application = fs::read_to_string(&application_path).unwrap();
        compile(&compiler, &application, &application_path, "server").unwrap();
        compile(&compiler, &application, &application_path, "client").unwrap();
    }
}
