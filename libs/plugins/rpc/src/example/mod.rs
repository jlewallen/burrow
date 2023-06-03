use std::{collections::HashSet, marker::PhantomData};

use anyhow::{Context, Result};
use kernel::{get_my_session, EntityGid, Entry, Surroundings};
use plugins_core::tools;
use tracing::{debug, info, span, trace, warn, Level};

use crate::proto::{
    AgentProtocol, AlwaysErrorsServer, Completed, DefaultResponses, EntityJson, EntityKey,
    LookupBy, Message, Payload, PayloadMessage, Query, QueryMessage, Sender, Server,
    ServerProtocol, SessionKey,
};

pub trait Inbox<T, R> {
    fn deliver(&mut self, message: &T, replies: &mut Sender<R>) -> Result<()>;
}

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

    pub fn handle(&mut self, _message: &Payload) -> Result<()> {
        trace!("(handle) {:?}", _message);

        Ok(())
    }
}

impl Inbox<PayloadMessage, QueryMessage> for ExampleAgent {
    fn deliver(
        &mut self,
        message: &PayloadMessage,
        replies: &mut Sender<QueryMessage>,
    ) -> Result<()> {
        self.agent.apply(message, replies)?;

        self.handle(message.body())?;

        Ok(())
    }
}

pub struct SessionServer {}

impl SessionServer {
    pub fn new_for_my_session() -> Result<Self> {
        Ok(Self {})
    }
}

impl SessionServer {
    fn lookup_one(&self, lookup: &LookupBy) -> Result<(LookupBy, Option<(Entry, EntityJson)>)> {
        let session = get_my_session().with_context(|| "SessionServer::lookup_one")?;
        let entry = match lookup {
            LookupBy::Key(key) => session.entry(&kernel::LookupBy::Key(&key.into()))?,
            LookupBy::Gid(gid) => session.entry(&kernel::LookupBy::Gid(&EntityGid::new(*gid)))?,
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
    session_key: SessionKey,
    server_tx: mpsc::Sender<ChannelMessage>,
    agent_tx: mpsc::Sender<ChannelMessage>,
    rx_agent: mpsc::Receiver<ChannelMessage>,
    server: ServerProtocol,
    _marker: PhantomData<P>,
}

#[derive(Debug)]
enum ChannelMessage {
    Query(Option<Query>),
    Payload(Payload),
}

async fn process_payload<T: Inbox<PayloadMessage, QueryMessage>>(
    session_key: &SessionKey,
    payload: Payload,
    agent_tx: &mpsc::Sender<ChannelMessage>,
    agent: &mut T,
) -> Result<()> {
    let mut to_server: Sender<_> = Default::default();
    let message = session_key.message(payload);
    agent.deliver(&message, &mut to_server)?;

    for sending in to_server.into_iter() {
        trace!("sending {:?}", sending);
        agent_tx
            .send(ChannelMessage::Query(sending.into_body()))
            .await?;
    }

    Ok(())
}

async fn process_query(
    session_key: &SessionKey,
    query: Option<Query>,
    server_tx: &mpsc::Sender<ChannelMessage>,
    mut server: &mut ServerProtocol,
) -> Result<Completed> {
    fn apply_query(
        server: &mut ServerProtocol,
        message: &Message<Option<Query>>,
        session_server: &dyn Server,
    ) -> Result<Sender<PayloadMessage>> {
        let mut to_agent: Sender<_> = Default::default();
        server.apply(&message, &mut to_agent, session_server)?;

        Ok(to_agent)
    }

    let message = session_key.message(query);
    let to_agent: Sender<_> = if message.body().is_none() {
        apply_query(&mut server, &message, &AlwaysErrorsServer {})?
    } else {
        apply_query(&mut server, &message, &SessionServer::new_for_my_session()?)?
    };

    for sending in to_agent.into_iter() {
        trace!("sending {:?}", sending);
        server_tx
            .send(ChannelMessage::Payload(sending.into_body()))
            .await?;
    }

    Ok(server.completed())
}

impl TokioChannelServer<ExampleAgent> {
    pub async fn new() -> Self {
        let (agent_tx, rx_agent) = tokio::sync::mpsc::channel::<ChannelMessage>(4);
        let (server_tx, mut rx_server) = tokio::sync::mpsc::channel::<ChannelMessage>(4);

        let session_key = SessionKey::new("SessionKey");

        let server = ServerProtocol::new(session_key.clone());
        /*
        tokio::spawn({
            let session_key = session_key.clone();
            let server_tx = server_tx.clone();

            async move {
                let mut server = ServerProtocol::new(session_key.clone());

                // Agent is transmitting queries to us and we're receiving from them.
                while let Some(cm) = rx_agent.recv().await {
                    if let ChannelMessage::Query(query) = cm {
                        match process_query(&session_key, query, &server_tx, &mut server).await {
                            Err(e) => panic!("Processing query: {:?}", e),
                            Ok(()) => {}
                        }
                    }
                }
            }
        });
        */

        tokio::spawn({
            let session_key = session_key.clone();
            let agent_tx = agent_tx.clone();

            async move {
                let mut agent = ExampleAgent::new();

                // Server is transmitting paylods to us and we're receiving from them.
                while let Some(cm) = rx_server.recv().await {
                    if let ChannelMessage::Payload(payload) = cm {
                        match process_payload(&session_key, payload, &agent_tx, &mut agent).await {
                            Err(e) => warn!("Payload error: {:?}", e),
                            Ok(()) => {}
                        }
                    }
                }
            }
        });

        Self {
            session_key,
            server_tx,
            agent_tx,
            rx_agent,
            server,
            _marker: Default::default(),
        }
    }

    async fn drive(&mut self) -> Result<()> {
        // Agent is transmitting queries to us and we're receiving from them.
        while let Some(cm) = self.rx_agent.recv().await {
            if let ChannelMessage::Query(query) = cm {
                match process_query(&self.session_key, query, &self.server_tx, &mut self.server)
                    .await
                {
                    Err(e) => panic!("Processing query: {:?}", e),
                    Ok(completed) => match completed {
                        Completed::Busy => continue,
                        Completed::Continue => break,
                    },
                }
            }
        }

        Ok(())
    }

    pub async fn initialize(&mut self) -> Result<()> {
        self.agent_tx.send(ChannelMessage::Query(None)).await?;

        self.drive().await?;

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

        self.drive().await?;

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
