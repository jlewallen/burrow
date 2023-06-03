use anyhow::{anyhow, Context};
use std::{
    sync::{Arc, RwLock},
    time::Duration,
};
use tokio::{
    runtime::{self, Handle},
    sync::mpsc::{self, Receiver, Sender},
    time::interval,
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

#[derive(Debug)]
enum RpcMessage {
    Shutdown,
}

pub struct Task {
    rx: Option<Receiver<RpcMessage>>,
}

impl Task {
    pub async fn run(mut self) {
        let mut rx = self.rx.take().expect("No receiver");
        let mut interval = interval(Duration::from_millis(1000));

        loop {
            tokio::select! {
                _ = interval.tick() => info!("tick"),
                m = rx.recv() => {
                    match m {
                        Some(m) => debug!("{:?}", m),
                        None => debug!("empty receive"),
                    }
                    break;
                }
            }
        }

        info!("stopped");
    }
}

#[derive(Clone)]
struct RpcServer {
    tx: Sender<RpcMessage>,
    example: Arc<RwLock<example::TokioChannelServer<example::ExampleAgent>>>,
}

impl RpcServer {
    pub async fn new(handle: Handle) -> Result<Self> {
        let (tx, rx) = mpsc::channel::<RpcMessage>(4);

        let example = example::TokioChannelServer::<example::ExampleAgent>::new().await;
        let server = RpcServer {
            tx: tx.clone(),
            example: Arc::new(RwLock::new(example)),
        };

        let _task = handle.spawn(server.task(rx).run());

        Ok(server)
    }

    pub async fn initialize(&self) -> Result<()> {
        let mut example = self.example.write().map_err(|_| anyhow!("Lock error"))?;
        example.initialize().await?;

        Ok(())
    }

    pub async fn have_surroundings(&self, surroundings: &Surroundings) -> Result<()> {
        let mut example = self.example.write().map_err(|_| anyhow!("Lock error"))?;
        example
            .have_surroundings(surroundings, &self.server()?)
            .await?;

        Ok(())
    }

    fn task(&self, rx: Receiver<RpcMessage>) -> Task {
        Task { rx: Some(rx) }
    }

    fn server(&self) -> Result<SessionServer> {
        SessionServer::new_for_my_session()
    }

    pub async fn stop(&self) -> Result<()> {
        self.tx
            .send(RpcMessage::Shutdown)
            .await
            .with_context(|| "RpcMessage::Shutdown")?;

        let mut example = self.example.write().map_err(|_| anyhow!("Lock error"))?;
        example.stop().await.with_context(|| "Stopping agent")
    }
}

#[derive(Clone)]
struct SynchronousWrapper {
    server: RpcServer,
}

impl SynchronousWrapper {
    pub fn initialize(&self) -> Result<()> {
        let rt = runtime::Builder::new_current_thread().build()?;
        rt.block_on(self.server.initialize())
    }

    pub fn have_surroundings(&self, surroundings: &Surroundings) -> Result<()> {
        let rt = runtime::Builder::new_current_thread().build()?;
        rt.block_on(self.server.have_surroundings(surroundings))
    }

    pub fn stop(&self) -> Result<()> {
        info!("Sync::stop");
        let rt = runtime::Builder::new_current_thread().build()?;
        rt.block_on(self.server.stop())
    }
}

impl RpcPluginFactory {
    pub async fn start(handle: Handle) -> Result<Self> {
        let server = RpcServer::new(handle).await?;
        let server = SynchronousWrapper { server };

        Ok(Self { server })
    }
}

impl PluginFactory for RpcPluginFactory {
    fn create_plugin(&self) -> Result<Box<dyn Plugin>> {
        Ok(Box::new(RpcPlugin {
            server: self.server.clone(),
        }))
    }

    fn stop(&self) -> Result<()> {
        self.server.stop()
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

    fn stop(&self) -> Result<()> {
        // Server is stopped by the plugin factory.
        // self.server.stop()
        Ok(())
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
