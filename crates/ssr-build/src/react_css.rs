use std::borrow::Cow;
use std::path::Path;

use rolldown::ModuleType;
use rolldown::plugin::{HookLoadArgs, HookLoadOutput, HookLoadReturn, HookUsage, Plugin, SharedLoadPluginContext};

#[derive(Debug)]
pub(crate) struct ReactCssPlugin;

impl Plugin for ReactCssPlugin {
    fn name(&self) -> Cow<'static, str> { Cow::Borrowed("ssr-build-react-css") }

    fn register_hook_usage(&self) -> HookUsage { HookUsage::Load }

    async fn load(&self, _ctx: SharedLoadPluginContext, args: &HookLoadArgs<'_>) -> HookLoadReturn {
        if Path::new(args.id).extension().and_then(|value| value.to_str()) != Some("css") {
            return Ok(None);
        }
        Ok(Some(HookLoadOutput { code: "".into(), module_type: Some(ModuleType::Js), ..Default::default() }))
    }
}
