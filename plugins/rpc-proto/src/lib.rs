use anyhow::Result;
use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::*;

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
pub enum JsonValue {
    Null,
    Bool(bool),
    Number(JsonNumber),
    String(String),
    Array(Vec<JsonValue>),
    Object(HashMap<String, JsonValue>),
}

impl From<serde_json::Value> for JsonValue {
    fn from(value: serde_json::Value) -> Self {
        match value {
            serde_json::Value::Null => Self::Null,
            serde_json::Value::Bool(b) => Self::Bool(b),
            serde_json::Value::Number(n) => Self::Number(n.into()),
            serde_json::Value::String(s) => Self::String(s),
            serde_json::Value::Array(a) => Self::Array(a.into_iter().map(|i| i.into()).collect()),
            serde_json::Value::Object(o) => {
                Self::Object(o.into_iter().map(|(k, v)| (k, v.into())).collect())
            }
        }
    }
}

impl Into<serde_json::Value> for JsonValue {
    fn into(self) -> serde_json::Value {
        match self {
            JsonValue::Null => serde_json::Value::Null,
            JsonValue::Bool(bool) => serde_json::Value::Bool(bool),
            JsonValue::Number(n) => serde_json::Value::Number(n.into()),
            JsonValue::String(s) => serde_json::Value::String(s),
            JsonValue::Array(a) => {
                serde_json::Value::Array(a.into_iter().map(|i| i.into()).collect())
            }
            JsonValue::Object(v) => {
                serde_json::Value::Object(v.into_iter().map(|(k, v)| (k, v.into())).collect())
            }
        }
    }
}

#[derive(Serialize, Deserialize, Encode, Decode, Clone)]
pub enum JsonNumber {
    PosInt(u64),
    NegInt(i64),
    Float(f64),
}

impl PartialEq for JsonNumber {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::PosInt(l0), Self::PosInt(r0)) => l0 == r0,
            (Self::NegInt(l0), Self::NegInt(r0)) => l0 == r0,
            (Self::Float(l0), Self::Float(r0)) => l0 == r0,
            _ => false,
        }
    }
}

impl Eq for JsonNumber {}

impl From<serde_json::Number> for JsonNumber {
    fn from(value: serde_json::Number) -> Self {
        match (value.as_i64(), value.as_u64(), value.as_f64()) {
            (Some(i), _, _) => Self::NegInt(i),
            (_, Some(i), _) => Self::PosInt(i),
            (_, _, Some(f)) => Self::Float(f),
            (None, None, None) => {
                error!("Strange serde_json::Number");
                panic!("Strange serde_json::Number");
            }
        }
    }
}

impl Into<serde_json::Number> for JsonNumber {
    fn into(self) -> serde_json::Number {
        match self {
            JsonNumber::PosInt(i) => i.into(),
            JsonNumber::NegInt(i) => i.into(),
            JsonNumber::Float(f) => {
                warn!("Strange float?");
                serde_json::Number::from_f64(f).expect("Non-finite number")
            }
        }
    }
}

#[derive(Serialize, Deserialize, Encode, Decode, PartialEq, Eq, Clone)]
pub struct EntityJson(JsonValue);

impl std::fmt::Debug for EntityJson {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("EntityJson").finish()
    }
}

impl TryInto<serde_json::Value> for EntityJson {
    type Error = serde_json::Error;

    fn try_into(self) -> Result<serde_json::Value, Self::Error> {
        Ok(self.0.into())
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
        Ok(Self(entity.to_json_value()?.into()))
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
