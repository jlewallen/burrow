use anyhow::Result;
use bincode::{Decode, Encode};
use kernel::{Incoming, JsonReply, ToJson};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, rc::Rc};
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

impl Into<EntityKey> for kernel::EntityKey {
    fn into(self) -> EntityKey {
        EntityKey(self.into())
    }
}

impl Into<kernel::EntityKey> for EntityKey {
    fn into(self) -> kernel::EntityKey {
        kernel::EntityKey::from_string(self.0)
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
            JsonNumber::Float(f) => serde_json::Number::from_f64(f).expect("Non-finite number"),
        }
    }
}

#[derive(Serialize, Deserialize, Encode, Decode, PartialEq, Eq, Clone)]
pub struct Json(JsonValue);

impl std::fmt::Debug for Json {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("EntityJson").finish()
    }
}

impl Into<serde_json::Value> for Json {
    fn into(self) -> serde_json::Value {
        self.0.into()
    }
}

impl From<serde_json::Value> for Json {
    fn from(value: serde_json::Value) -> Self {
        Self(value.into())
    }
}

impl ToJson for Json {
    fn to_json(&self) -> std::result::Result<serde_json::Value, serde_json::Error> {
        Ok(self.0.clone().into())
    }
}

#[derive(Debug, Serialize, Deserialize, Encode, Decode, PartialEq, Eq, Clone)]
pub struct EntityUpdate {
    pub key: EntityKey,
    pub entity: Json,
}

impl EntityUpdate {
    pub fn new(key: EntityKey, entity: Json) -> Self {
        Self { key, entity }
    }
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
    Update(EntityUpdate),
    Raise(Audience, Json),
    Schedule(String, i64, Json),
    Complete,
    Effect(Effect),
    // Lookup(u32, Vec<LookupBy>),
}

impl Into<kernel::Audience> for Audience {
    fn into(self) -> kernel::Audience {
        match self {
            Audience::Nobody => kernel::Audience::Nobody,
            Audience::Everybody => kernel::Audience::Everybody,
            Audience::Individuals(keys) => {
                kernel::Audience::Individuals(keys.into_iter().map(|k| k.into()).collect())
            }
            Audience::Area(area) => kernel::Audience::Area(area.into()),
        }
    }
}

impl From<kernel::Audience> for Audience {
    fn from(value: kernel::Audience) -> Self {
        match value {
            kernel::Audience::Nobody => Audience::Nobody,
            kernel::Audience::Everybody => Audience::Everybody,
            kernel::Audience::Individuals(keys) => {
                Audience::Individuals(keys.into_iter().map(|k| k.into()).collect())
            }
            kernel::Audience::Area(area) => Audience::Area(area.into()),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Encode, Decode, PartialEq, Eq, Clone)]
pub enum Audience {
    Nobody,
    Everybody,
    Individuals(Vec<EntityKey>),
    Area(EntityKey),
}

#[derive(Debug, Serialize, Deserialize, Encode, Decode, PartialEq, Eq, Clone)]
pub enum Effect {
    Reply(Json),
}

impl TryFrom<kernel::Effect> for Effect {
    type Error = anyhow::Error;

    fn try_from(value: kernel::Effect) -> std::result::Result<Self, Self::Error> {
        match value {
            kernel::Effect::Reply(reply) => Ok(Self::Reply(reply.to_json()?.try_into()?)),
            _ => todo!(),
        }
    }
}

impl Into<kernel::Effect> for Effect {
    fn into(self) -> kernel::Effect {
        match self {
            Effect::Reply(value) => {
                kernel::Effect::Reply(Rc::new(JsonReply::from(<Json as Into<
                    serde_json::Value,
                >>::into(value))))
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Encode, Decode, PartialEq, Eq, Clone)]
pub enum Surroundings {
    Living {
        world: EntityKey,
        living: EntityKey,
        area: EntityKey,
    },
}

impl TryFrom<&kernel::Entry> for Json {
    type Error = anyhow::Error;

    fn try_from(value: &kernel::Entry) -> Result<Self, Self::Error> {
        let entity = value.entity();
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
pub struct IncomingMessage {
    pub key: String,
    pub serialized: Vec<u8>,
}

impl IncomingMessage {
    pub fn from(incoming: &Incoming) -> Self {
        Self {
            key: incoming.key.clone(),
            serialized: incoming.serialized.clone(),
        }
    }
}

impl Into<Incoming> for IncomingMessage {
    fn into(self) -> Incoming {
        Incoming {
            key: self.key,
            serialized: self.serialized,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Encode, Decode, PartialEq, Eq, Clone)]
pub enum Payload {
    Initialize, /* Complete */
    Resolved(Vec<(LookupBy, Option<Json>)>),
    Surroundings(Surroundings),
    Deliver(IncomingMessage),
    Evaluate(String),
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

impl<S> Sender<S> {
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

pub mod prelude {
    pub use super::Payload;
    pub use super::Query;
}
