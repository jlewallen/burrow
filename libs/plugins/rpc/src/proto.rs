use serde::{Deserialize, Serialize};
use tracing::*;

pub type SessionKey = String;

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Clone, Debug)]
pub struct EntityKey(String);

impl EntityKey {
    pub fn new(key: String) -> Self {
        Self(key)
    }
}

impl From<&kernel::EntityKey> for EntityKey {
    fn from(value: &kernel::EntityKey) -> Self {
        Self(value.to_string())
    }
}

impl Into<kernel::EntityKey> for &EntityKey {
    fn into(self) -> kernel::EntityKey {
        kernel::EntityKey::new(&self.0)
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq)]
pub struct EntityJson(serde_json::Value);

impl std::fmt::Debug for EntityJson {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("EntityJson").finish()
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct EntityUpdate {
    entity_key: EntityKey,
    entity: EntityJson,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Event {
    Arrived,
    Left,
    Held,
    Dropped,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Reply {
    Done,
    NotFound,
    Impossible,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Find {}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Try {
    CanMove,
    Moved,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Permission {}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Hook {}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum LookupBy {
    Key(EntityKey),
    Gid(u64),
}

/*
impl<'a> Into<kernel::LookupBy<'a>> for &LookupBy {
    fn into(self) -> kernel::LookupBy<'a> {
        match self {
            LookupBy::Key(key) => kernel::LookupBy::Key(&key.into()),
            LookupBy::Gid(gid) => kernel::LookupBy::Gid(&EntityGid::new(*gid)),
        }
    }
}
*/

const DEFAULT_DEPTH: u32 = 2;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Query {
    Complete,

    Update(EntityUpdate),
    Raise(Event),
    Chain(String),
    Reply(Reply),

    Permission(Try),

    Lookup(u32, Vec<LookupBy>),
    Find(Find),

    Try(Try),
}

#[derive(Serialize, Deserialize)]
pub struct Message<B> {
    session_key: SessionKey,
    body: B,
}

impl<B> Message<B> {
    pub fn body(&self) -> &B {
        &self.body
    }
}

pub type QueryMessage = Message<Option<Query>>;

impl std::fmt::Debug for QueryMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Query").field("body", &self.body).finish()
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Surroundings {
    Living {
        world: EntityKey,
        living: EntityKey,
        area: EntityKey,
    },
}

impl TryFrom<&kernel::Entry> for EntityJson {
    type Error = anyhow::Error;

    fn try_from(value: &kernel::Entry) -> Result<Self, Self::Error> {
        let entity = value.entity()?;
        Ok(Self(entity.to_json_value()?))
    }
}

impl TryFrom<&kernel::Surroundings> for Surroundings {
    type Error = anyhow::Error;

    fn try_from(value: &kernel::Surroundings) -> Result<Self, Self::Error> {
        match value {
            kernel::Surroundings::Living {
                world,
                living,
                area,
            } => Ok(Self::Living {
                world: world.key().into(),
                living: living.key().into(),
                area: area.key().into(),
            }),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Payload {
    Initialize(String), /* Complete */

    Surroundings(Surroundings),
    Evaluate(String, Surroundings), /* Reply */

    Resolved(Vec<(LookupBy, Option<EntityJson>)>),
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
pub struct Sender<S> {
    queue: Vec<S>,
}

impl<S> Default for Sender<S> {
    fn default() -> Self {
        Self {
            queue: Default::default(),
        }
    }
}

#[allow(dead_code)]
impl<S> Sender<S>
where
    S: std::fmt::Debug,
{
    pub fn send(&mut self, message: S) -> anyhow::Result<()> {
        debug!("Sending {:?}", &message);
        self.queue.push(message);

        Ok(())
    }

    pub fn iter(&self) -> impl Iterator<Item = &S> {
        self.queue.iter()
    }

    pub fn clear(&mut self) {
        self.queue.clear()
    }

    pub fn pop(&mut self) -> Option<S> {
        self.queue.pop()
    }
}

impl<B> Sender<Message<B>> {
    #[cfg(test)]
    pub fn bodies(&self) -> impl Iterator<Item = &B> {
        self.queue.iter().map(|m| &m.body)
    }
}

enum Transition<S, M> {
    None,
    Direct(S),
    Send(M, S),
    SendOnly(M),
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
            Transition::SendOnly(m) => Transition::<S, O>::SendOnly(f(m)),
        }
    }
}

#[derive(Debug)]
struct Machine<S> {
    state: S,
}

#[allow(dead_code)]
impl<S> Machine<S>
where
    S: std::fmt::Debug,
{
    fn apply<M>(
        &mut self,
        transition: Transition<S, M>,
        sender: &mut Sender<M>,
    ) -> anyhow::Result<()>
    where
        M: std::fmt::Debug,
    {
        match transition {
            Transition::None => {
                debug!("(none) {:?}", &self.state);
                Ok(())
            }
            Transition::Direct(state) => {
                debug!("(direct) {:?} -> {:?}", &self.state, &state);
                self.state = state;
                Ok(())
            }
            Transition::Send(sending, state) => {
                debug!("(send) {:?}", &sending);
                sender.send(sending)?;
                debug!("(send) {:?} -> {:?}", &self.state, &state);
                self.state = state;
                Ok(())
            }
            Transition::SendOnly(sending) => {
                debug!("(send-only) {:?}", &sending);
                sender.send(sending)?;
                Ok(())
            }
        }
    }
}

#[allow(dead_code)]
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
        Resolving,
    }

    type PluginMachine = Machine<PluginState>;

    impl PluginMachine {
        fn new() -> Self {
            Self {
                state: PluginState::Uninitialized,
            }
        }
    }

    #[derive(Debug)]
    pub struct PluginProtocol {
        session_key: Option<String>,
        machine: PluginMachine,
    }

    impl PluginProtocol {
        pub fn new() -> Self {
            Self {
                session_key: None,
                machine: PluginMachine::new(),
            }
        }

        pub fn new_with_session_key(session_key: String) -> Self {
            Self {
                session_key: Some(session_key),
                machine: PluginMachine::new(),
            }
        }

        pub fn session_key(&self) -> Option<&str> {
            self.session_key.as_deref()
        }

        pub fn message<B>(&self, body: B) -> Message<B> {
            Message {
                session_key: self.session_key.clone().expect("A session key is required"),
                body,
            }
        }
    }

    impl PluginProtocol {
        pub fn apply(
            &mut self,
            message: &PayloadMessage,
            sender: &mut Sender<QueryMessage>,
        ) -> Result<()> {
            let transition = self.handle(message).map_message(|m| QueryMessage {
                session_key: self.session_key.as_ref().unwrap().clone(),
                body: Some(m),
            });

            self.machine.apply(transition, sender)?;

            Ok(())
        }

        fn handle(&mut self, message: &PayloadMessage) -> PluginTransition {
            match (&self.machine.state, &message.body) {
                (PluginState::Uninitialized, Payload::Initialize(session_key)) => {
                    self.session_key = Some(session_key.to_owned());

                    PluginTransition::Direct(PluginState::Initialized)
                }
                (PluginState::Initialized, Payload::Surroundings(surroundings)) => {
                    let keys = match &surroundings {
                        Surroundings::Living {
                            world,
                            living,
                            area,
                        } => vec![world, living, area],
                    };

                    let lookups = keys.into_iter().map(|k| LookupBy::Key(k.clone())).collect();
                    let lookup = Query::Lookup(DEFAULT_DEPTH, lookups);

                    PluginTransition::Send(lookup, PluginState::Resolving)
                }
                (PluginState::Resolving, Payload::Resolved(_entities)) => PluginTransition::None,
                (PluginState::Failed, payload) => {
                    warn!("(failed) {:?}", &payload);

                    PluginTransition::None
                }
                (state, message) => {
                    warn!("(failing) {:?} {:?}", state, message);

                    PluginTransition::Direct(PluginState::Failed)
                }
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
            let mut proto = PluginProtocol::new_with_session_key("session-key".to_owned());

            assert_eq!(proto.machine.state, PluginState::Uninitialized);

            let session_key = proto.session_key().unwrap().to_owned();
            let initialize = Payload::Initialize(session_key);
            let mut sender = Default::default();
            proto.apply(&proto.message(initialize), &mut sender)?;

            assert_eq!(proto.machine.state, PluginState::Initialized);
            assert!(sender.queue.is_empty());

            Ok(())
        }
    }
}

#[allow(dead_code)]
mod server {
    use super::*;
    use anyhow::{anyhow, Result};

    type ServerTransition = Transition<ServerState, Payload>;

    #[derive(Debug, PartialEq, Eq)]
    enum ServerState {
        Initializing,
        Initialized,
        Failed,
    }

    type ServerMachine = Machine<ServerState>;

    impl ServerMachine {
        fn new() -> Self {
            Self {
                state: ServerState::Initializing,
            }
        }
    }

    #[derive(Debug)]
    pub struct ServerProtocol {
        session_key: String,
        machine: ServerMachine,
    }

    impl ServerProtocol {
        pub fn new() -> Self {
            Self {
                session_key: "session-key".to_owned(),
                machine: ServerMachine::new(),
            }
        }

        pub fn session_key(&self) -> &str {
            &self.session_key
        }

        pub fn message<B>(&self, body: B) -> Message<B> {
            Message {
                session_key: self.session_key.clone(),
                body,
            }
        }
    }

    impl ServerProtocol {
        pub fn apply(
            &mut self,
            message: &QueryMessage,
            sender: &mut Sender<PayloadMessage>,
            server: &dyn Server,
        ) -> Result<()> {
            let transition = self
                .handle(message, server)?
                .map_message(|m| PayloadMessage {
                    session_key: self.session_key.clone(),
                    body: m,
                });

            self.machine.apply(transition, sender)?;

            Ok(())
        }

        fn handle(
            &mut self,
            message: &QueryMessage,
            server: &dyn Server,
        ) -> Result<ServerTransition> {
            match (&self.machine.state, &message.body) {
                (ServerState::Initializing, _) => Ok(ServerTransition::Send(
                    Payload::Initialize(message.session_key.clone()),
                    ServerState::Initialized,
                )),
                (ServerState::Initialized, Some(Query::Lookup(depth, lookup))) => {
                    let resolved = server.lookup(*depth, lookup)?;

                    Ok(ServerTransition::SendOnly(Payload::Resolved(resolved)))
                }
                (ServerState::Failed, query) => {
                    warn!("(failed) {:?}", &query);

                    Ok(ServerTransition::None)
                }
                (state, message) => {
                    warn!("(failing) {:?} {:?}", state, message);

                    Ok(ServerTransition::Direct(ServerState::Failed))
                }
            }
        }
    }

    pub trait Server {
        fn lookup(
            &self,
            depth: u32,
            lookup: &Vec<LookupBy>,
        ) -> Result<Vec<(LookupBy, Option<EntityJson>)>>;
    }

    pub struct AlwaysErrorsServer {}

    impl Server for AlwaysErrorsServer {
        fn lookup(
            &self,
            _depth: u32,
            _lookup: &Vec<LookupBy>,
        ) -> Result<Vec<(LookupBy, Option<EntityJson>)>> {
            Err(anyhow!("This server always errors"))
        }
    }

    #[cfg(test)]
    mod tests {
        use anyhow::Result;
        #[allow(unused_imports)]
        use tracing::*;

        use crate::proto::{server::ServerState, EntityJson, LookupBy, Payload};

        use super::{Server, ServerProtocol};

        struct DummyServer {}

        impl Server for DummyServer {
            fn lookup(
                &self,
                _depth: u32,
                _lookup: &Vec<LookupBy>,
            ) -> Result<Vec<(LookupBy, Option<EntityJson>)>> {
                Ok(vec![])
            }
        }

        #[tokio::test]
        async fn test_initialize() -> anyhow::Result<()> {
            let mut proto = ServerProtocol::new();

            assert_eq!(proto.machine.state, ServerState::Initializing);

            let mut sender = Default::default();
            let start = proto.message(None);
            proto.apply(&start, &mut sender, &DummyServer {})?;

            assert_eq!(proto.machine.state, ServerState::Initialized);

            assert_eq!(
                sender.bodies().next(),
                Some(&Payload::Initialize("session-key".to_owned()))
            );

            Ok(())
        }
    }
}

pub use plugin::PluginProtocol;
pub use server::AlwaysErrorsServer;
pub use server::Server;
pub use server::ServerProtocol;
