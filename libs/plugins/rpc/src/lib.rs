use anyhow::anyhow;
use std::sync::{Arc, RwLock};
use tokio::{
    runtime::Handle,
    sync::mpsc::{self, Receiver, Sender},
};

mod example;
mod proto;

use plugins_core::library::plugin::*;

pub use example::ExampleAgent;
pub use example::SessionServer;
pub use example::{InProcessServer, TokioChannelServer};

pub struct RpcPluginFactory {
    server: SynchronousWrapper,
}

enum RpcMessage {}

pub struct Task {
    _rx: Receiver<RpcMessage>,
}

impl Task {
    pub async fn run(self) {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
            info!("tick");
        }
    }
}

#[derive(Clone)]
struct RpcServer {
    _tx: Sender<RpcMessage>,
    example: Arc<RwLock<example::TokioChannelServer<example::ExampleAgent>>>,
}

impl RpcServer {
    pub async fn initialize(&self) -> Result<()> {
        let mut example = self
            .example
            .write()
            .map_err(|_| anyhow!("Read lock error"))?;
        example.initialize()?;

        Ok(())
    }

    pub async fn have_surroundings(&self, surroundings: &Surroundings) -> Result<()> {
        let mut example = self
            .example
            .write()
            .map_err(|_| anyhow!("Read lock error"))?;
        example.have_surroundings(surroundings, &self.server()?)?;

        Ok(())
    }

    fn task(&self, rx: Receiver<RpcMessage>) -> Task {
        Task { _rx: rx }
    }

    fn server(&self) -> Result<SessionServer> {
        SessionServer::new_for_my_session()
    }
}

#[derive(Clone)]
struct SynchronousWrapper {
    handle: Handle,
    server: RpcServer,
}

impl SynchronousWrapper {
    pub fn initialize(&self) -> Result<()> {
        self.handle.block_on(self.server.initialize())
    }

    pub fn have_surroundings(&self, surroundings: &Surroundings) -> Result<()> {
        self.handle
            .block_on(self.server.have_surroundings(surroundings))
    }
}

impl RpcPluginFactory {
    pub async fn start(handle: Handle) -> Result<Self> {
        let (tx, rx) = mpsc::channel::<RpcMessage>(32);

        let example = example::TokioChannelServer::<example::ExampleAgent>::new().await;

        let server = RpcServer {
            _tx: tx.clone(),
            example: Arc::new(RwLock::new(example)),
        };
        let _task = handle.spawn(server.task(rx).run());
        let server = SynchronousWrapper { handle, server };

        Ok(Self { server })
    }
}

impl Drop for RpcPluginFactory {
    fn drop(&mut self) {
        info!("~RpcPluginFactory");
    }
}

impl PluginFactory for RpcPluginFactory {
    fn create_plugin(&self) -> Result<Box<dyn Plugin>> {
        Ok(Box::new(RpcPlugin {
            server: self.server.clone(),
        }))
    }
}

pub struct RpcPlugin {
    server: SynchronousWrapper,
}

impl RpcPlugin {}

impl Plugin for RpcPlugin {
    fn plugin_key() -> &'static str
    where
        Self: Sized,
    {
        "rpc"
    }

    #[tracing::instrument(name = "rpc-initialize", skip(self))]
    fn initialize(&mut self) -> Result<()> {
        self.server.initialize()
    }

    #[tracing::instrument(name = "rpc-register", skip_all)]
    fn register_hooks(&self, _hooks: &ManagedHooks) -> Result<()> {
        Ok(())
    }

    #[tracing::instrument(name = "rpc-surroundings", skip_all)]
    fn have_surroundings(&self, surroundings: &Surroundings) -> Result<()> {
        self.server.have_surroundings(surroundings)
    }
}

impl ParsesActions for RpcPlugin {
    fn try_parse_action(&self, _i: &str) -> EvaluationResult {
        Err(EvaluationError::ParseFailed)
    }
}

#[cfg(test)]
#[ctor::ctor]
fn initialize_tests() {
    plugins_core::log_test();
}
