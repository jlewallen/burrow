use plugins_core::library::plugin::*;

mod proto;

#[derive(Default)]
pub struct RpcPluginFactory {}

impl PluginFactory for RpcPluginFactory {
    fn create_plugin(&self) -> Result<Box<dyn Plugin>> {
        Ok(Box::new(RpcPlugin::default()))
    }
}

#[derive(Default)]
pub struct RpcPlugin {}

impl RpcPlugin {}

impl Plugin for RpcPlugin {
    fn plugin_key() -> &'static str
    where
        Self: Sized,
    {
        "rpc"
    }

    fn initialize(&mut self) -> Result<()> {
        Ok(())
    }

    fn register_hooks(&self, _hooks: &ManagedHooks) -> Result<()> {
        Ok(())
    }

    fn have_surroundings(&self, _surroundings: &Surroundings) -> Result<()> {
        Ok(())
    }
}

impl ParsesActions for RpcPlugin {
    fn try_parse_action(&self, _i: &str) -> EvaluationResult {
        Err(EvaluationError::ParseFailed)
    }
}

pub mod actions {
    // use kernel::*;
}

mod parser {
    // use kernel::*;
    // use plugins_core::library::parser::*;
}
