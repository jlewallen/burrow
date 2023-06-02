use std::{collections::HashSet, marker::PhantomData, rc::Rc};

use anyhow::{Context, Result};
use kernel::{get_my_session, ActiveSession, EntityGid, Entry, Surroundings};
use plugins_core::tools;
use tracing::{debug, info, span, trace, warn, Level};

use crate::proto::{
    AgentProtocol, AlwaysErrorsServer, DefaultResponses, EntityJson, EntityKey, LookupBy, Message,
    Payload, PayloadMessage, Query, QueryMessage, Sender, Server, ServerProtocol, SessionKey,
};

#[derive(Debug)]
pub struct ExampleAgent {
    agent: AgentProtocol<DefaultResponses>,
}

impl ExampleAgent {
    pub fn new() -> Self {
        Self {
            agent: AgentProtocol::new(),
        }
    }

    pub fn deliver(
        &mut self,
        message: &PayloadMessage,
        replies: &mut Sender<QueryMessage>,
    ) -> Result<()> {
        info!("{:?}", message.body());

        self.agent.apply(message, replies)?;

        self.handle(message.body())?;

        Ok(())
    }

    pub fn handle(&mut self, _message: &Payload) -> Result<()> {
        debug!("(handle) {:?}", _message);

        Ok(())
    }
}

pub struct SessionServer {
    session: Rc<dyn ActiveSession>,
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
    Payload(Payload),
}

fn apply_query(
    server: &mut ServerProtocol,
    message: Message<Option<Query>>,
    session_server: &dyn Server,
) -> Result<Sender<PayloadMessage>> {
    let mut to_agent: Sender<_> = Default::default();
    server
        .apply(&message, &mut to_agent, session_server)
        .expect("Server protocol error");

    Ok(to_agent)
}

impl TokioChannelServer<ExampleAgent> {
    pub async fn new() -> Self {
        let (agent_tx, mut rx_agent) = tokio::sync::mpsc::channel::<ChannelMessage>(4);
        let (server_tx, mut rx_server) = tokio::sync::mpsc::channel::<ChannelMessage>(4);

        let session_key = SessionKey::new("SessionKey");

        tokio::spawn({
            let session_key = session_key.clone();
            let server_tx = server_tx.clone();

            async move {
                let mut server = ServerProtocol::new(session_key.clone());

                // Agent is transmitting queries to us and we're receiving from them.
                while let Some(cm) = rx_agent.recv().await {
                    match cm {
                        ChannelMessage::Query(query) => {
                            let message = session_key.message(query);
                            let to_agent: Sender<_> = if message.body().is_none() {
                                apply_query(&mut server, message, &AlwaysErrorsServer {})
                                    .expect("Apply failed")
                            } else {
                                apply_query(
                                    &mut server,
                                    message,
                                    &SessionServer::new_for_my_session()
                                        .expect("Session server error"),
                                )
                                .expect("Apply failed")
                            };

                            for sending in to_agent.into_iter() {
                                info!("sending {:?}", sending);
                                match server_tx
                                    .send(ChannelMessage::Payload(sending.into_body()))
                                    .await
                                {
                                    Err(e) => warn!("Error sending: {:?}", e),
                                    Ok(_) => {}
                                }
                            }
                        }
                        ChannelMessage::Payload(_) => unimplemented!(),
                    }
                }
            }
        });

        tokio::spawn({
            let session_key = session_key.clone();
            let agent_tx = agent_tx.clone();

            async move {
                let mut agent = ExampleAgent::new();

                // Server is transmitting paylods to us and we're receiving from them.
                while let Some(cm) = rx_server.recv().await {
                    match cm {
                        ChannelMessage::Payload(payload) => {
                            let mut to_server: Sender<_> = Default::default();
                            let message = session_key.message(payload);
                            agent
                                .deliver(&message, &mut to_server)
                                .with_context(|| format!("{:?}", message))
                                .expect("Server protocol error");

                            for sending in to_server.into_iter() {
                                info!("sending {:?}", sending);
                                match agent_tx
                                    .send(ChannelMessage::Query(sending.into_body()))
                                    .await
                                {
                                    Err(e) => warn!("Error sending: {:?}", e),
                                    Ok(_) => {}
                                }
                            }
                        }
                        ChannelMessage::Query(_) => unimplemented!(),
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

    pub async fn initialize(&mut self) -> Result<()> {
        self.agent_tx.send(ChannelMessage::Query(None)).await?;

        Ok(())
    }

    pub async fn have_surroundings(
        &mut self,
        surroundings: &Surroundings,
        _server: &dyn Server,
    ) -> Result<()> {
        self.server_tx
            .send(ChannelMessage::Payload(Payload::Surroundings(
                surroundings.try_into()?,
            )))
            .await?;

        Ok(())
    }
}

pub struct InProcessServer<P> {
    session_key: SessionKey,
    server: ServerProtocol,
    agent: P,
}

impl InProcessServer<ExampleAgent> {
    pub fn new(session_key: SessionKey) -> Self {
        Self {
            session_key: session_key.clone(),
            server: ServerProtocol::new(session_key),
            agent: ExampleAgent::new(),
        }
    }

    pub fn initialize(&mut self) -> Result<()> {
        self.handle(self.session_key.message(None), &AlwaysErrorsServer {})
    }

    pub fn have_surroundings(
        &mut self,
        surroundings: &Surroundings,
        server: &dyn Server,
    ) -> Result<()> {
        let payload = Payload::Surroundings(surroundings.try_into()?);
        self.send(&self.session_key.message(payload), server)
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
        self.agent.deliver(message, to_server)
    }
}
