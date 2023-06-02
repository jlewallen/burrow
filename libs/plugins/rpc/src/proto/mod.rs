use serde::{Deserialize, Serialize};
use tracing::*;

mod agent;
mod fsm;
mod server;

pub use fsm::Sender;

pub use agent::AgentProtocol;
pub use agent::AgentResponses;
pub use agent::DefaultResponses;
pub use server::AlwaysErrorsServer;
pub use server::Server;
pub use server::ServerProtocol;

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

impl From<&EntityKey> for kernel::EntityKey {
    fn from(value: &EntityKey) -> Self {
        kernel::EntityKey::new(&value.0)
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct EntityJson(serde_json::Value);

impl std::fmt::Debug for EntityJson {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("EntityJson").finish()
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct EntityUpdate {
    entity_key: EntityKey,
    entity: EntityJson,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum Event {
    Arrived,
    Left,
    Held,
    Dropped,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum Reply {
    Done,
    NotFound,
    Impossible,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum Find {}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum Try {
    CanMove,
    Moved,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum Permission {}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
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

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
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

    pub fn into_body(self) -> B {
        self.body
    }
}

pub type QueryMessage = Message<Option<Query>>;

impl std::fmt::Debug for QueryMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Query").field("body", &self.body).finish()
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
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

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
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
