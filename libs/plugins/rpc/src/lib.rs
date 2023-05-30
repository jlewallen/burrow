use std::cell::RefCell;

use example::SessionServer;
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
    example: RefCell<example::InProcessServer<example::ExamplePlugin>>,
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

#[allow(dead_code)]
#[allow(unused_imports)]
mod example {
    use anyhow::Result;
    use kernel::{get_my_session, ActiveSession, EntityGid, Entry, SessionRef, Surroundings};
    use plugins_core::tools;
    use tracing::{debug, info, span, Level};

    use crate::proto::{
        AlwaysErrorsServer, EntityJson, EntityKey, LookupBy, Payload, PayloadMessage,
        PluginProtocol, Query, QueryMessage, Sender, Server, ServerProtocol,
    };

    pub struct ExamplePlugin {
        plugin: PluginProtocol,
    }

    impl ExamplePlugin {
        pub fn new() -> Self {
            Self {
                plugin: PluginProtocol::new(),
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
            info!("{:?}", _message);

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

        fn lookup(
            &self,
            lookup: &Vec<LookupBy>,
        ) -> Result<Vec<(LookupBy, Option<(Entry, EntityJson)>)>> {
            Ok(lookup
                .iter()
                .map(|lookup| {
                    debug!("lookup({:?})", lookup);

                    // This is awkward because of the generic lifetime on kernel::LookupBy
                    let entry = match lookup {
                        LookupBy::Key(key) => {
                            self.session.entry(&kernel::LookupBy::Key(&key.into()))?
                        }
                        LookupBy::Gid(gid) => self
                            .session
                            .entry(&kernel::LookupBy::Gid(&EntityGid::new(*gid)))?,
                    };

                    match entry {
                        Some(entry) => {
                            Ok((lookup.clone(), Some((entry.clone(), (&entry).try_into()?))))
                        }
                        None => Ok((lookup.clone(), None)),
                    }
                })
                .collect::<Result<Vec<(_, Option<_>)>>>()?)
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

        pub fn into_with<F>(self, mut f: F) -> Result<Self>
        where
            F: FnMut(LookupBy) -> Result<(LookupBy, Option<(Entry, EntityJson)>)>,
        {
            info!("querying {}", self.queue.len());

            let adding = self
                .queue
                .into_iter()
                .map(|lookup| f(lookup))
                .collect::<Result<Vec<_>>>()?;

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
                .flat_map(|v| {
                    v.into_iter()
                        .map(|key| LookupBy::Key(EntityKey::new(key.to_string())))
                })
                .map(|v| v.into())
                .collect();

            let entities = self.entities.into_iter().chain(adding).collect();

            Ok(Self { queue, entities })
        }
    }

    impl Server for SessionServer {
        fn lookup(
            &self,
            depth: u32,
            lookup: &Vec<LookupBy>,
        ) -> Result<Vec<(LookupBy, Option<EntityJson>)>> {
            let done = (0..depth).into_iter().fold(
                Ok::<_, anyhow::Error>(FoldToDepth::new(lookup)),
                |acc, depth| match acc {
                    Ok(acc) => {
                        let _span = span!(Level::INFO, "folding", depth = depth).entered();
                        acc.into_with(|lookup| self.lookup_one(&lookup))
                    }
                    Err(e) => Err(e),
                },
            )?;

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

    impl Default for InProcessServer<ExamplePlugin> {
        fn default() -> Self {
            Self {
                server: ServerProtocol::new(),
                plugin: ExamplePlugin::new(),
            }
        }
    }

    impl InProcessServer<ExamplePlugin> {
        pub fn new() -> Self {
            Self {
                server: ServerProtocol::new(),
                plugin: ExamplePlugin::new(),
            }
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
