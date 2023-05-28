use serde::{Deserialize, Serialize};
use std::marker::PhantomData;
use tracing::{debug, info};

pub type SessionKey = String;

pub type EntityKey = String;

pub type EntityJson = serde_json::Value;

#[derive(Debug, Serialize, Deserialize)]
pub struct EntityUpdate {
    entity_key: EntityKey,
    entity: EntityJson,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Event {
    Arrived,
    Left,
    Held,
    Dropped,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Reply {
    Done,
    NotFound,
    Impossible,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Find {}

#[derive(Debug, Serialize, Deserialize)]
pub enum Try {
    CanMove,
    Moved,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Permission {}

#[derive(Debug, Serialize, Deserialize)]
pub enum Hook {}

#[derive(Debug, Serialize, Deserialize)]
pub enum LookupBy {
    Key(EntityKey),
    Gid(u64),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Query {
    Complete,

    Update(EntityUpdate),
    Raise(Event),
    Chain(String),
    Reply(Reply),

    Permission(Try),

    Lookup(LookupBy),
    Find(Find),

    Try(Try),
}

#[derive(Serialize, Deserialize)]
pub struct Message<B> {
    session_key: SessionKey,
    body: B,
}

impl<B> Message<B> {
    fn into_tuple(self) -> (SessionKey, B) {
        (self.session_key, self.body)
    }
}

pub type QueryMessage = Message<Option<Query>>;

impl std::fmt::Debug for QueryMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Query").field("body", &self.body).finish()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Surroundings {
    Living {
        world: EntityJson,
        living: EntityJson,
        area: EntityJson,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Payload {
    Initialize(String), /* Complete */

    Evaluate(String, Surroundings), /* Reply */

    Entity(Option<EntityJson>),
    Found(Vec<EntityJson>),

    Permission(Permission),

    Hook(Hook),
}

pub type PayloadMessage = Message<Payload>;

impl std::fmt::Debug for PayloadMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Payload").field("body", &self.body).finish()
    }
}

#[derive(Debug)]
struct Sender<S> {
    _phantom: PhantomData<S>,
}

impl<S> Default for Sender<S> {
    fn default() -> Self {
        Self {
            _phantom: Default::default(),
        }
    }
}

impl<S> Sender<S>
where
    S: std::fmt::Debug,
{
    async fn send(&self, message: S) -> anyhow::Result<()> {
        debug!("Sending {:?}", &message);

        Ok(())
    }
}

enum Transition<S, M> {
    None,
    Direct(S),
    Send(M, S),
}

impl<S, M> Transition<S, M> {
    fn map_message<O, F>(self, mut f: F) -> Transition<S, O>
    where
        F: FnMut(M) -> O,
    {
        match self {
            Transition::None => Transition::<S, O>::None,
            Transition::Direct(s) => Transition::<S, O>::Direct(s),
            Transition::Send(m, s) => Transition::<S, O>::Send(f(m), s),
        }
    }
}

#[derive(Debug)]
struct Machine<S, M> {
    state: S,
    sender: Sender<M>,
}

impl<S, M> Machine<S, M>
where
    S: std::fmt::Debug,
    M: std::fmt::Debug,
{
    async fn apply(&mut self, transition: Transition<S, M>) -> anyhow::Result<()> {
        match transition {
            Transition::None => {
                info!("(none) {:?}", &self.state);
                Ok(())
            }
            Transition::Direct(state) => {
                info!("(direct) {:?} -> {:?}", &self.state, &state);
                self.state = state;
                Ok(())
            }
            Transition::Send(sending, state) => {
                info!("(send) {:?}", &sending);
                self.sender.send(sending).await?;
                info!("(send) {:?} -> {:?}", &self.state, &state);
                self.state = state;
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod plugin {
    use super::*;
    use super::{Payload, PayloadMessage, Query};
    use anyhow::Result;
    use tracing::warn;

    type PluginTransition = Transition<PluginState, Query>;

    #[derive(Debug, PartialEq, Eq)]
    enum PluginState {
        Uninitialized,
        Initialized,
        Failed,
    }

    type PluginMachine = Machine<PluginState, QueryMessage>;

    impl Default for PluginMachine {
        fn default() -> Self {
            Self {
                state: PluginState::Uninitialized,
                sender: Default::default(),
            }
        }
    }

    #[derive(Debug)]
    pub struct PluginProtocol {
        session_key: Option<String>,
        machine: PluginMachine,
    }

    impl PluginProtocol {
        fn new(session_key: String) -> Self {
            Self {
                session_key: Some(session_key),
                ..Default::default()
            }
        }

        fn session_key(&self) -> Option<&str> {
            self.session_key.as_deref()
        }

        fn message<B>(&self, body: B) -> Message<B> {
            Message {
                session_key: self.session_key.clone().expect("A session key is required"),
                body,
            }
        }
    }

    impl Default for PluginProtocol {
        fn default() -> Self {
            Self {
                session_key: None,
                machine: PluginMachine::default(),
            }
        }
    }

    impl PluginProtocol {
        pub async fn apply(&mut self, message: PayloadMessage) -> Result<()> {
            let transition = self.handle(message).map_message(|m| QueryMessage {
                session_key: self.session_key.as_ref().unwrap().clone(),
                body: Some(m),
            });

            self.machine.apply(transition).await?;

            Ok(())
        }

        fn handle(&mut self, message: PayloadMessage) -> PluginTransition {
            let (_session_key, payload) = message.into_tuple();
            match (&self.machine.state, payload) {
                (PluginState::Uninitialized, Payload::Initialize(session_key)) => {
                    self.session_key = Some(session_key);

                    PluginTransition::Direct(PluginState::Initialized)
                }
                (PluginState::Initialized, _) => todo!(),
                (PluginState::Failed, payload) => {
                    warn!("(failed) {:?}", &payload);

                    PluginTransition::None
                }
                (_, _) => PluginTransition::Direct(PluginState::Failed),
            }
        }
    }

    #[cfg(test)]
    mod tests {
        #[allow(unused_imports)]
        use tracing::*;

        use crate::proto::{plugin::PluginState, Payload};

        use super::PluginProtocol;

        #[tokio::test]
        async fn test_initialize() -> anyhow::Result<()> {
            let mut proto = PluginProtocol::new("session-key".to_owned());

            assert_eq!(proto.machine.state, PluginState::Uninitialized);

            let session_key = proto.session_key().unwrap().to_owned();
            let initialize = Payload::Initialize(session_key);
            proto.apply(proto.message(initialize)).await?;

            assert_eq!(proto.machine.state, PluginState::Initialized);

            Ok(())
        }
    }
}

mod server {
    use super::*;
    use anyhow::Result;
    use tracing::*;

    type ServerTransition = Transition<ServerState, Payload>;

    #[derive(Debug, PartialEq, Eq)]
    enum ServerState {
        Initializing,
        Initialized,
        Failed,
    }

    type ServerMachine = Machine<ServerState, PayloadMessage>;

    impl Default for ServerMachine {
        fn default() -> Self {
            Self {
                state: ServerState::Initializing,
                sender: Default::default(),
            }
        }
    }

    #[derive(Debug)]
    pub struct ServerProtocol {
        #[allow(dead_code)]
        session_key: String,
        machine: ServerMachine,
    }

    impl ServerProtocol {
        #[allow(dead_code)]
        fn session_key(&self) -> &str {
            &self.session_key
        }

        fn message<B>(&self, body: B) -> Message<B> {
            Message {
                session_key: self.session_key.clone(),
                body,
            }
        }
    }

    impl Default for ServerProtocol {
        fn default() -> Self {
            Self {
                session_key: "session-key".to_owned(),
                machine: ServerMachine::default(),
            }
        }
    }

    impl ServerProtocol {
        pub async fn apply(&mut self, message: QueryMessage) -> Result<()> {
            let transition = self.handle(message).map_message(|m| PayloadMessage {
                session_key: self.session_key.clone(),
                body: m,
            });

            self.machine.apply(transition).await?;

            Ok(())
        }

        fn handle(&mut self, message: QueryMessage) -> ServerTransition {
            let (session_key, query) = message.into_tuple();
            match (&self.machine.state, query) {
                (ServerState::Initializing, _) => ServerTransition::Send(
                    Payload::Initialize(session_key.clone()),
                    ServerState::Initialized,
                ),
                (ServerState::Failed, query) => {
                    warn!("(failed) {:?}", &query);

                    ServerTransition::None
                }
                (_, _) => ServerTransition::Direct(ServerState::Failed),
            }
        }
    }

    #[cfg(test)]
    mod tests {
        #[allow(unused_imports)]
        use tracing::*;

        use crate::proto::server::ServerState;

        use super::ServerProtocol;

        #[tokio::test]
        async fn test_initialize() -> anyhow::Result<()> {
            let mut proto = ServerProtocol::default();

            assert_eq!(proto.machine.state, ServerState::Initializing);

            let start = proto.message(None);
            proto.apply(start).await?;

            assert_eq!(proto.machine.state, ServerState::Initialized);

            Ok(())
        }
    }
}
