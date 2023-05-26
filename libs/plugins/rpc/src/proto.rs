use serde::{Deserialize, Serialize};

pub type SessionKey = String;

pub type EntityKey = String;

pub type EntityJson = serde_json::Value;

#[derive(Serialize, Deserialize)]
pub struct EntityUpdate {
    entity_key: EntityKey,
    entity: EntityJson,
}

#[derive(Serialize, Deserialize)]
pub enum Event {
    Arrived,
    Left,
    Held,
    Dropped,
}

#[derive(Serialize, Deserialize)]
pub enum Reply {
    Done,
    NotFound,
    Impossible,
}

#[derive(Serialize, Deserialize)]
pub enum Find {}

#[derive(Serialize, Deserialize)]
pub enum Try {
    CanMove,
    Moved,
}

#[derive(Serialize, Deserialize)]
pub enum Permission {}

#[derive(Serialize, Deserialize)]
pub enum Hook {}

#[derive(Serialize, Deserialize)]
pub enum LookupBy {
    Key(EntityKey),
    Gid(u64),
}

#[derive(Serialize, Deserialize)]
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
pub struct QueryMessage {
    session_key: SessionKey,
    query: Query,
}

#[derive(Serialize, Deserialize)]
pub enum Surroundings {
    Living {
        world: EntityJson,
        living: EntityJson,
        area: EntityJson,
    },
}

#[derive(Serialize, Deserialize)]
pub enum Payload {
    Initialize, /* Complete */

    Evaluate(String, Surroundings), /* Reply */

    Entity(Option<EntityJson>),
    Found(Vec<EntityJson>),

    Permission(Permission),

    Hook(Hook),
}

#[derive(Serialize, Deserialize)]
pub struct PayloadMessage {
    session_key: SessionKey,
    payload: Payload,
}
