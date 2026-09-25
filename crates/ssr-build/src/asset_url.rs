use std::borrow::Cow;
use std::io;
use std::path::{Path, PathBuf};

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
    route: String,
}

impl AssetUrlPlugin {
    pub(crate) fn new(root: PathBuf, route: String) -> Self {
        Self { root, route }
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
        if !path.is_absolute() || !path.starts_with(&self.root) || path.canonicalize()? != path {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("asset must be a file inside application root: {clean}"),
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
