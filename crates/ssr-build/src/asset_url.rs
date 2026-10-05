use std::borrow::Cow;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use ordered_json::Value;
use rolldown::ModuleType;
use rolldown::plugin::{
    HookLoadArgs, HookLoadOutput, HookLoadReturn, HookResolveFileUrlArgs, HookResolveFileUrlReturn,
    HookUsage, Plugin, PluginHookMeta, PluginOrder, SharedLoadPluginContext,
};
use rolldown_plugin_utils::emit_asset;

use crate::public_url;

#[derive(Debug)]
pub(crate) struct AssetUrlPlugin {
    root: PathBuf,
    packages: PathBuf,
    route: String,
    errors: Arc<Mutex<Vec<String>>>,
}

impl AssetUrlPlugin {
    /// Assets are files under the application root or under the package directory, whose
    /// packages ship their own assets. Rolldown reports a failed load only as "plugin threw an
    /// error", without the plugin's message, so each load error is also kept in `errors`, which
    /// the build adds to its error.
    pub(crate) fn new(
        root: PathBuf,
        packages: PathBuf,
        route: String,
        errors: Arc<Mutex<Vec<String>>>,
    ) -> Self {
        Self {
            root,
            packages,
            route,
            errors,
        }
    }
}

impl Plugin for AssetUrlPlugin {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("ssr-build-assets")
    }

    fn register_hook_usage(&self) -> HookUsage {
        HookUsage::Load | HookUsage::ResolveFileUrl
    }

    fn load_meta(&self) -> Option<PluginHookMeta> {
        Some(PluginHookMeta {
            order: Some(PluginOrder::Pre),
        })
    }

    async fn load(&self, ctx: SharedLoadPluginContext, args: &HookLoadArgs<'_>) -> HookLoadReturn {
        let result = self.load_asset(ctx, args).await;
        if let Err(error) = &result {
            self.errors
                .lock()
                .map_err(|_| io::Error::other("asset load error record is poisoned"))?
                .push(format!("{error:#}"));
        }
        result
    }

    async fn resolve_file_url(
        &self,
        _ctx: &rolldown::plugin::PluginContext,
        args: &HookResolveFileUrlArgs<'_>,
    ) -> HookResolveFileUrlReturn {
        let filename = args.file_name;
        if !filename.starts_with("assets/")
            || filename
                .split('/')
                .any(|part| part.is_empty() || part == "." || part == "..")
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("invalid emitted asset path: {filename}"),
            )
            .into());
        }
        let url = public_url(&self.route, filename);
        Ok(Some(Value::string(&url).compact()))
    }
}

impl AssetUrlPlugin {
    async fn load_asset(
        &self,
        ctx: SharedLoadPluginContext,
        args: &HookLoadArgs<'_>,
    ) -> HookLoadReturn {
        let clean = args
            .id
            .split(['?', '#'])
            .next()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "empty asset path"))?;
        let extension = Path::new(clean)
            .extension()
            .and_then(|value| value.to_str());
        let listed = matches!(
            extension,
            Some(
                "png" | "svg" | "jpg" | "gif" | "webp" | "avif" | "ico" | "woff" | "woff2" | "ttf"
            )
        );
        if !listed && !matches!(args.asserted_module_type, Some(ModuleType::Asset)) {
            return Ok(None);
        }
        if clean != args.id {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("asset query or fragment is unsupported: {}", args.id),
            )
            .into());
        }
        let path = Path::new(clean);
        if !path.is_absolute()
            || !(path.starts_with(&self.root) || path.starts_with(&self.packages))
            || path.canonicalize()? != path
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "asset must be a file inside application root or package directory: {clean}"
                ),
            )
            .into());
        }
        path.file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("asset filename is not UTF-8: {clean}"),
                )
            })?;
        let reference_id = emit_asset(&ctx, clean, |error| {
            io::Error::new(error.kind(), format!("cannot read asset {clean}: {error}")).into()
        })
        .await?;
        ctx.associate_module_with_file_ref(args.id, &reference_id);
        Ok(Some(HookLoadOutput {
            code: format!("export default import.meta.ROLLDOWN_FILE_URL_{reference_id};").into(),
            module_type: Some(ModuleType::Js),
            ..Default::default()
        }))
    }
}
