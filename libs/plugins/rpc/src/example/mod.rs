use std::{collections::HashSet, marker::PhantomData};

use anyhow::Result;
use kernel::{get_my_session, ActiveSession, EntityGid, Entry, Surroundings};
use plugins_core::tools;
use tracing::{debug, info, span, trace, warn, Level};

use crate::proto::{
    AgentProtocol, AlwaysErrorsServer, DefaultResponses, EntityJson, EntityKey, LookupBy, Payload,
    PayloadMessage, Query, QueryMessage, Sender, Server, ServerProtocol,
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

use tokio::sync::mpsc;

#[allow(dead_code)]
pub struct TokioChannelServer<P> {
    server_tx: mpsc::Sender<ChannelMessage>,
    agent_tx: mpsc::Sender<ChannelMessage>,
    _marker: PhantomData<P>,
}

#[derive(Debug)]
enum ChannelMessage {
    Query(Option<Query>),
    Payload(crate::proto::Message<Payload>),
}

#[allow(dead_code)]
impl TokioChannelServer<ExampleAgent> {
    pub async fn new() -> Self {
        let (agent_tx, mut rx_agent) = tokio::sync::mpsc::channel::<ChannelMessage>(4);
        let (server_tx, mut rx_server) = tokio::sync::mpsc::channel::<ChannelMessage>(4);

        tokio::spawn({
            let server_tx = server_tx.clone();

            async move {
                let mut server = ServerProtocol::new();

                // Agent is transmitting queries to us and we're receiving from them.
                while let Some(cm) = rx_agent.recv().await {
                    match cm {
                        ChannelMessage::Query(query) => {
                            let mut to_agent: Sender<_> = Default::default();

                            {
                                // Scope for Session, which isn't Send.
                                let session_server = SessionServer::new_for_my_session()
                                    .expect("Session server error");
                                let message = server.message(query);
                                server
                                    .apply(&message, &mut to_agent, &session_server)
                                    .expect("Server protocol error");
                            }

                            for sending in to_agent.into_iter() {
                                match server_tx.send(ChannelMessage::Payload(sending)).await {
                                    Err(e) => warn!("Error sending: {:?}", e),
                                    Ok(_) => {}
                                }
                            }
                        }
                        ChannelMessage::Payload(_) => {}
                    }
                }
            }
        });

        tokio::spawn({
            let agent_tx = agent_tx.clone();

            async move {
                let mut agent = ExampleAgent::new();

                // Server is transmitting paylods to us and we're receiving from them.
                while let Some(cm) = rx_server.recv().await {
                    match cm {
                        ChannelMessage::Payload(payload) => {
                            let mut to_server: Sender<_> = Default::default();
                            let payload = payload.into_body();
                            let message = agent.message(payload);
                            agent
                                .deliver(&message, &mut to_server)
                                .expect("Server protocol error");

                            for sending in to_server.into_iter() {
                                match agent_tx
                                    .send(ChannelMessage::Query(sending.into_body()))
                                    .await
                                {
                                    Err(e) => warn!("Error sending: {:?}", e),
                                    Ok(_) => {}
                                }
                            }
                        }
                        ChannelMessage::Query(_) => {}
                    }
                }
            }
        });

        Self {
            server_tx,
            agent_tx,
            _marker: Default::default(),
        }
    }

    pub fn initialize(&mut self) -> Result<()> {
        /*
        while let Some(sending) = to_server.pop() {
            self.server.apply(&sending, &mut to_plugin, server)?;
            for message in to_plugin.iter() {
                self.deliver(message, &mut to_server)?;
            }
        }
        */
        // self.agent_tx.blocking_send(ChannelMessage::Query(None))?;

        Ok(())
    }

    pub fn have_surroundings(
        &mut self,
        _surroundings: &Surroundings,
        _server: &dyn Server,
    ) -> Result<()> {
        Ok(())
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

    fn handle(&mut self, query: QueryMessage, server: &dyn Server) -> Result<()> {
        let mut to_server: Sender<_> = Default::default();
        to_server.send(query)?;
        self.drain(to_server, server)
    }

    fn drain(&mut self, mut to_server: Sender<QueryMessage>, server: &dyn Server) -> Result<()> {
        let mut to_agent: Sender<_> = Default::default();

        while let Some(sending) = to_server.pop() {
            self.server.apply(&sending, &mut to_agent, server)?;
            for message in to_agent.iter() {
                self.deliver(message, &mut to_server)?;
            }
        }

        Ok(())
    }

    fn send(&mut self, message: &PayloadMessage, server: &dyn Server) -> Result<()> {
        let mut to_server: Sender<_> = Default::default();

        self.deliver(message, &mut to_server)?;

        self.drain(to_server, server)
    }

    fn deliver(
        &mut self,
        message: &PayloadMessage,
        to_server: &mut Sender<QueryMessage>,
    ) -> Result<()> {
        trace!("{:?}", message);
        self.plugin.deliver(message, to_server)
    }
}
