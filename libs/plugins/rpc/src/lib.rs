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
        let mut example = example::InMemoryExamplePlugin::new();

        example.initialize()?;

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

#[allow(dead_code)]
mod example {
    use anyhow::Result;

    use crate::proto::{PayloadMessage, PluginProtocol, ServerProtocol};

    pub struct InMemoryExamplePlugin {
        plugin: PluginProtocol,
        server: ServerProtocol,
    }

    impl InMemoryExamplePlugin {
        pub fn new() -> Self {
            Self {
                plugin: PluginProtocol::new(),
                server: ServerProtocol::new(),
            }
        }

        pub fn initialize(&mut self) -> Result<()> {
            let mut sender = Default::default();
            let start = self.server.message(None);
            self.server.apply(start, &mut sender)?;

            Ok(())
        }

        fn handle(&self, _payload: PayloadMessage) -> Result<()> {
            todo!()
        }
    }
}

#[cfg(test)]
#[ctor::ctor]
fn initialize_tests() {
    plugins_core::log_test();
}
