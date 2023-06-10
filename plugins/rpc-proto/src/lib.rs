use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use tracing::*;

mod agent;
mod fsm;
mod server;

pub use agent::AgentProtocol;
pub use agent::AgentResponses;
pub use agent::DefaultResponses;
pub use server::AlwaysErrorsServices;
pub use server::Completed;
pub use server::ServerProtocol;
pub use server::Services;

pub trait Inbox<T, R> {
    fn deliver(&mut self, message: &T, replies: &mut Sender<R>) -> anyhow::Result<()>;
}

#[derive(Serialize, Deserialize, Encode, Decode, PartialEq, Eq, Hash, Clone, Debug)]
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

#[derive(Serialize, Deserialize, Encode, Decode, PartialEq, Eq, Clone)]
pub struct EntityJson(String);

impl std::fmt::Debug for EntityJson {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("EntityJson").finish()
    }
}

#[derive(Debug, Serialize, Deserialize, Encode, Decode, PartialEq, Eq, Clone)]
pub struct EntityUpdate {
    entity_key: EntityKey,
    entity: EntityJson,
}

#[derive(Debug, Serialize, Deserialize, Encode, Decode, PartialEq, Eq, Clone)]
pub enum Event {
    Arrived,
    Left,
    Held,
    Dropped,
}

#[derive(Debug, Serialize, Deserialize, Encode, Decode, PartialEq, Eq, Clone)]
pub enum Reply {
    Done,
    NotFound,
    Impossible,
}

#[derive(Debug, Serialize, Deserialize, Encode, Decode, PartialEq, Eq, Clone)]
pub enum Find {}

#[derive(Debug, Serialize, Deserialize, Encode, Decode, PartialEq, Eq, Clone)]
pub enum Try {
    CanMove,
    Moved,
}

#[derive(Debug, Serialize, Deserialize, Encode, Decode, PartialEq, Eq, Clone)]
pub enum Permission {}

#[derive(Debug, Serialize, Deserialize, Encode, Decode, PartialEq, Eq, Clone)]
pub enum Hook {}

#[derive(Debug, Serialize, Deserialize, Encode, Decode, PartialEq, Eq, Clone)]
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

#[derive(Debug, Serialize, Deserialize, Encode, Decode, PartialEq, Eq, Clone)]
pub enum Query {
    Bootstrap,

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

#[derive(Debug, Serialize, Deserialize, Encode, Decode, PartialEq, Eq, Clone)]
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
        Ok(Self(entity.to_json_value()?.to_string())) // TODO Ew
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

#[derive(Debug, Serialize, Deserialize, Encode, Decode, PartialEq, Eq, Clone)]
pub enum Payload {
    Initialize, /* Complete */

    Surroundings(Surroundings),
    Evaluate(String, Surroundings), /* Reply */

    Resolved(Vec<(LookupBy, Option<EntityJson>)>),
    Found(Vec<EntityJson>),

    Permission(Permission),

    Hook(Hook),
}

#[derive(Debug)]
pub struct Sender<S> {
    pub queue: Vec<S>,
}

impl<S> Default for Sender<S> {
    fn default() -> Self {
        Self {
            queue: Default::default(),
        }
    }
}

impl<S> Sender<S>
where
    S: std::fmt::Debug,
{
    pub fn send(&mut self, message: S) -> anyhow::Result<()> {
        self.queue.push(message);

        Ok(())
    }

    pub fn iter(&self) -> impl Iterator<Item = &S> {
        self.queue.iter()
    }

    pub fn into_iter(self) -> impl Iterator<Item = S> {
        self.queue.into_iter()
    }

    pub fn clear(&mut self) {
        self.queue.clear()
    }

    pub fn pop(&mut self) -> Option<S> {
        self.queue.pop()
    }
}
