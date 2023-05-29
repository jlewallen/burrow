use std::cell::RefCell;

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
pub struct RpcPlugin {
    example: RefCell<example::InMemoryExamplePlugin>,
}

impl RpcPlugin {}

impl Plugin for RpcPlugin {
    fn plugin_key() -> &'static str
    where
        Self: Sized,
    {
        "rpc"
    }

    fn initialize(&mut self) -> Result<()> {
        let _span = span!(Level::INFO, "rpc").entered();

        let mut example = self.example.borrow_mut();
        example.initialize()?;

        Ok(())
    }

    fn register_hooks(&self, _hooks: &ManagedHooks) -> Result<()> {
        Ok(())
    }

    fn have_surroundings(&self, surroundings: &Surroundings) -> Result<()> {
        let _span = span!(Level::INFO, "rpc").entered();

        let mut example = self.example.borrow_mut();
        example.have_surroundings(surroundings)?;

        Ok(())
    }
}

impl ParsesActions for RpcPlugin {
    fn try_parse_action(&self, _i: &str) -> EvaluationResult {
        Err(EvaluationError::ParseFailed)
    }
}

#[allow(dead_code)]
#[allow(unused_imports)]
mod example {
    use anyhow::Result;
    use kernel::Surroundings;
    use tracing::info;

    use crate::proto::{
        Payload, PayloadMessage, PluginProtocol, Query, QueryMessage, Sender, ServerProtocol,
    };

    pub struct InMemoryExamplePlugin {
        plugin: PluginProtocol,
        server: ServerProtocol,
    }

    impl Default for InMemoryExamplePlugin {
        fn default() -> Self {
            Self {
                plugin: PluginProtocol::new(),
                server: ServerProtocol::new(),
            }
        }
    }

    impl InMemoryExamplePlugin {
        pub fn new() -> Self {
            Self {
                plugin: PluginProtocol::new(),
                server: ServerProtocol::new(),
            }
        }

        pub fn initialize(&mut self) -> Result<()> {
            self.handle(self.server.message(None))
        }

        pub fn have_surroundings(&mut self, surroundings: &Surroundings) -> Result<()> {
            let payload = Payload::Surroundings(surroundings.try_into()?);
            self.send(&self.plugin.message(payload))
        }

        pub fn handle(&mut self, query: QueryMessage) -> Result<()> {
            let mut to_server: Sender<_> = Default::default();

            to_server.send(query)?;

            self.drain(to_server)
        }

        pub fn drain(&mut self, mut to_server: Sender<QueryMessage>) -> Result<()> {
            let mut to_plugin: Sender<_> = Default::default();

            while let Some(sending) = to_server.pop() {
                self.server.apply(&sending, &mut to_plugin)?;
                for message in to_plugin.iter() {
                    info!("{:?}", message);
                    self.plugin.apply(message, &mut to_server)?;
                }
            }

            Ok(())
        }

        pub fn send(&mut self, message: &PayloadMessage) -> Result<()> {
            let mut to_server: Sender<_> = Default::default();

            info!("{:?}", message);
            self.plugin.apply(message, &mut to_server)?;

            self.drain(to_server)
        }
    }
}

#[cfg(test)]
#[ctor::ctor]
fn initialize_tests() {
    plugins_core::log_test();
}
