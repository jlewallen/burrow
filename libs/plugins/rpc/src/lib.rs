use std::cell::RefCell;
use tokio::{
    runtime::Handle,
    sync::mpsc::{self, Receiver, Sender},
};

use example::SessionServer;
use plugins_core::library::plugin::*;

mod proto;

pub struct RpcPluginFactory {
    server: SynchronousWrapper,
}

enum RpcMessage {}

#[derive(Clone)]
struct RpcServer {
    tx: Sender<RpcMessage>,
}

impl RpcServer {
    pub async fn initialize(&self) -> Result<()> {
        todo!()
    }

    pub async fn have_surroundings(&self, surroundings: Surroundings) -> Result<()> {
        todo!()
    }
}

#[derive(Clone)]
struct SynchronousWrapper {
    handle: Handle,
    server: RpcServer,
}

impl SynchronousWrapper {
    pub fn initialize(&self) -> Result<()> {
        todo!()
    }

    pub fn have_surroundings(&self, surroundings: Surroundings) -> Result<()> {
        todo!()
    }
}

async fn start_server(rx: Receiver<RpcMessage>) -> Result<()> {
    loop {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        info!("tick");
    }
}

impl RpcPluginFactory {
    pub fn start(handle: Handle) -> Result<Self> {
        let (tx, rx) = mpsc::channel::<RpcMessage>(32);
        let server = RpcServer { tx: tx.clone() };
        let _task = handle.spawn(start_server(rx));
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
            example: Default::default(),
            server: self.server.clone(),
        }))
    }
}

pub struct RpcPlugin {
    example: RefCell<example::InProcessServer<example::ExampleAgent>>,
    server: SynchronousWrapper,
}

impl RpcPlugin {
    fn server(&self) -> Result<SessionServer> {
        SessionServer::new_for_my_session()
    }
}

impl Plugin for RpcPlugin {
    fn plugin_key() -> &'static str
    where
        Self: Sized,
    {
        "rpc"
    }

    #[tracing::instrument(name = "rpc-initialize", skip(self))]
    fn initialize(&mut self) -> Result<()> {
        let mut example = self.example.borrow_mut();
        example.initialize()?;

        self.server.initialize()?;

        Ok(())
    }

    #[tracing::instrument(name = "rpc-register", skip_all)]
    fn register_hooks(&self, _hooks: &ManagedHooks) -> Result<()> {
        Ok(())
    }

    #[tracing::instrument(name = "rpc-surroundings", skip_all)]
    fn have_surroundings(&self, surroundings: &Surroundings) -> Result<()> {
        let mut example = self.example.borrow_mut();
        example.have_surroundings(surroundings, &self.server()?)?;

        Ok(())
    }
}

impl ParsesActions for RpcPlugin {
    fn try_parse_action(&self, _i: &str) -> EvaluationResult {
        Err(EvaluationError::ParseFailed)
    }
}

mod example {
    use std::collections::HashSet;

    use anyhow::Result;
    use kernel::{get_my_session, ActiveSession, EntityGid, Entry, Surroundings};
    use plugins_core::tools;
    use tracing::{debug, info, span, Level};

    use crate::proto::{
        AgentProtocol, AlwaysErrorsServer, DefaultResponses, EntityJson, EntityKey, LookupBy,
        Payload, PayloadMessage, QueryMessage, Sender, Server, ServerProtocol,
    };

    pub struct ExampleAgent {
        plugin: AgentProtocol<DefaultResponses>,
    }

    impl ExampleAgent {
        pub fn new() -> Self {
            Self {
                plugin: AgentProtocol::new(),
            }
        }

        pub fn message(&self, body: Payload) -> PayloadMessage {
            self.plugin.message(body)
        }

        pub fn deliver(
            &mut self,
            message: &PayloadMessage,
            replies: &mut Sender<QueryMessage>,
        ) -> Result<()> {
            self.plugin.apply(message, replies)?;

            self.handle(message.body())?;

            Ok(())
        }

        pub fn handle(&mut self, _message: &Payload) -> Result<()> {
            debug!("(handle) {:?}", _message);

            Ok(())
        }
    }

    pub struct SessionServer {
        session: std::rc::Rc<dyn ActiveSession>,
    }

    impl SessionServer {
        pub fn new_for_my_session() -> Result<Self> {
            Ok(Self {
                session: get_my_session()?,
            })
        }
    }

    impl SessionServer {
        fn lookup_one(&self, lookup: &LookupBy) -> Result<(LookupBy, Option<(Entry, EntityJson)>)> {
            let entry = match lookup {
                LookupBy::Key(key) => self.session.entry(&kernel::LookupBy::Key(&key.into()))?,
                LookupBy::Gid(gid) => self
                    .session
                    .entry(&kernel::LookupBy::Gid(&EntityGid::new(*gid)))?,
            };

            match entry {
                Some(entry) => Ok((lookup.clone(), Some((entry.clone(), (&entry).try_into()?)))),
                None => Ok((lookup.clone(), None)),
            }
        }
    }

    #[derive(Default)]
    struct FoldToDepth {
        queue: Vec<LookupBy>,
        entities: Vec<(LookupBy, Option<(Entry, EntityJson)>)>,
    }

    impl FoldToDepth {
        pub fn new(prime: &[LookupBy]) -> Self {
            Self {
                queue: prime.into(),
                ..Default::default()
            }
        }

        pub fn into_with<F>(self, f: F) -> Result<Self>
        where
            F: FnMut(LookupBy) -> Result<(LookupBy, Option<(Entry, EntityJson)>)>,
        {
            debug!(queue = self.queue.len(), "discovering");

            let have: HashSet<&kernel::EntityKey> = self
                .entities
                .iter()
                .filter_map(|(_lookup, maybe)| maybe.as_ref().map(|m| m.0.key()))
                .collect();

            let adding = self.queue.into_iter().map(f).collect::<Result<Vec<_>>>()?;

            let queue = adding
                .iter()
                .map(|(_lookup, maybe)| match maybe {
                    Some((entry, _)) => {
                        let mut keys = Vec::new();
                        keys.extend(tools::get_contained_keys(entry)?);
                        keys.extend(tools::get_occupant_keys(entry)?);
                        keys.extend(tools::get_adjacent_keys(entry)?);
                        Ok(keys)
                    }
                    None => Ok(vec![]),
                })
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .flat_map(|v| v.into_iter())
                .collect::<HashSet<kernel::EntityKey>>()
                .into_iter()
                .filter_map(|key| have.get(&key).map_or(Some(key), |_| None))
                .map(|key| LookupBy::Key(EntityKey::new(key.to_string())))
                .collect();

            let entities = self.entities.into_iter().chain(adding).collect();

            Ok(Self { queue, entities })
        }
    }

    impl Server for SessionServer {
        fn lookup(
            &self,
            depth: u32,
            lookup: &[LookupBy],
        ) -> Result<Vec<(LookupBy, Option<EntityJson>)>> {
            let done = (0..depth).fold(
                Ok::<_, anyhow::Error>(FoldToDepth::new(lookup)),
                |acc, depth| match acc {
                    Ok(acc) => {
                        let _span = span!(Level::INFO, "folding", depth = depth).entered();
                        acc.into_with(|lookup| self.lookup_one(&lookup))
                    }
                    Err(e) => Err(e),
                },
            )?;

            info!(nentities = done.entities.len(), depth = depth, "lookup");

            Ok(done
                .entities
                .into_iter()
                .map(|(lookup, maybe)| (lookup, maybe.map(|m| m.1)))
                .collect())
        }
    }

    pub struct InProcessServer<P> {
        server: ServerProtocol,
        plugin: P,
    }

    impl Default for InProcessServer<ExampleAgent> {
        fn default() -> Self {
            Self {
                server: ServerProtocol::new(),
                plugin: ExampleAgent::new(),
            }
        }
    }

    impl InProcessServer<ExampleAgent> {
        #[allow(dead_code)]
        pub fn new() -> Self {
            Default::default()
        }

        pub fn initialize(&mut self) -> Result<()> {
            self.handle(self.server.message(None), &AlwaysErrorsServer {})
        }

        pub fn have_surroundings(
            &mut self,
            surroundings: &Surroundings,
            server: &dyn Server,
        ) -> Result<()> {
            let payload = Payload::Surroundings(surroundings.try_into()?);
            self.send(&self.plugin.message(payload), server)
        }

        pub fn handle(&mut self, query: QueryMessage, server: &dyn Server) -> Result<()> {
            let mut to_server: Sender<_> = Default::default();
            to_server.send(query)?;
            self.drain(to_server, server)
        }

        pub fn drain(
            &mut self,
            mut to_server: Sender<QueryMessage>,
            server: &dyn Server,
        ) -> Result<()> {
            let mut to_plugin: Sender<_> = Default::default();

            while let Some(sending) = to_server.pop() {
                self.server.apply(&sending, &mut to_plugin, server)?;
                for message in to_plugin.iter() {
                    self.deliver(message, &mut to_server)?;
                }
            }

            Ok(())
        }

        pub fn send(&mut self, message: &PayloadMessage, server: &dyn Server) -> Result<()> {
            let mut to_server: Sender<_> = Default::default();

            self.deliver(message, &mut to_server)?;

            self.drain(to_server, server)
        }

        fn deliver(
            &mut self,
            message: &PayloadMessage,
            to_server: &mut Sender<QueryMessage>,
        ) -> Result<()> {
            debug!("{:?}", message);
            self.plugin.deliver(message, to_server)
        }
    }
}

#[cfg(test)]
#[ctor::ctor]
fn initialize_tests() {
    plugins_core::log_test();
}
