use anyhow::Result;
use bincode::{Decode, Encode};
use kernel::{EffectReply, Incoming, JsonReply, TaggedJson, TaggedJsonError, ToJson};
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

impl From<kernel::EntityKey> for EntityKey {
    fn from(value: kernel::EntityKey) -> Self {
        EntityKey(value.into())
    }
}

impl From<EntityKey> for kernel::EntityKey {
    fn from(value: EntityKey) -> Self {
        Self::from_string(value.0)
    }
}

#[derive(Debug, Serialize, Deserialize, Encode, Decode, PartialEq, Eq, Clone)]
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

impl From<JsonValue> for serde_json::Value {
    fn from(value: JsonValue) -> Self {
        match value {
            JsonValue::Null => Self::Null,
            JsonValue::Bool(bool) => Self::Bool(bool),
            JsonValue::Number(n) => Self::Number(n.into()),
            JsonValue::String(s) => Self::String(s),
            JsonValue::Array(a) => Self::Array(a.into_iter().map(|i| i.into()).collect()),
            JsonValue::Object(v) => {
                Self::Object(v.into_iter().map(|(k, v)| (k, v.into())).collect())
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

impl std::fmt::Debug for JsonNumber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PosInt(arg0) => f.debug_tuple("PosInt").field(arg0).finish(),
            Self::NegInt(arg0) => f.debug_tuple("NegInt").field(arg0).finish(),
            Self::Float(arg0) => f.debug_tuple("Float").field(arg0).finish(),
        }
    }
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

impl From<JsonNumber> for serde_json::Number {
    fn from(value: JsonNumber) -> Self {
        match value {
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

impl From<Json> for serde_json::Value {
    fn from(value: Json) -> Self {
        value.0.into()
    }
}

impl From<serde_json::Value> for Json {
    fn from(value: serde_json::Value) -> Self {
        Self(value.into())
    }
}

impl ToJson for Json {
    fn to_tagged_json(&self) -> std::result::Result<TaggedJson, TaggedJsonError> {
        let value: serde_json::Value = self.0.clone().into();
        Ok(TaggedJson::new_from(value)?)
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

impl From<Audience> for kernel::Audience {
    fn from(value: Audience) -> Self {
        match value {
            Audience::Nobody => Self::Nobody,
            Audience::Everybody => Self::Everybody,
            Audience::Individuals(keys) => {
                Self::Individuals(keys.into_iter().map(|k| k.into()).collect())
            }
            Audience::Area(area) => Self::Area(area.into()),
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
            kernel::Effect::Reply(reply) => Ok(Self::Reply(
                reply.to_tagged_json()?.into_tagged().try_into()?,
            )),
            _ => todo!(),
        }
    }
}

impl From<Effect> for kernel::Effect {
    fn from(value: Effect) -> Self {
        match value {
            Effect::Reply(value) => Self::Reply(EffectReply::Instance(Rc::new(JsonReply::from(
                <Json as Into<serde_json::Value>>::into(value),
            )))),
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
    pub value: JsonValue,
}

impl IncomingMessage {
    pub fn from(incoming: &Incoming) -> Self {
        Self {
            key: incoming.key.clone(),
            value: incoming.value.clone().into(),
        }
    }
}

impl From<IncomingMessage> for Incoming {
    fn from(value: IncomingMessage) -> Self {
        Self {
            key: value.key,
            value: value.value.into(),
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

    pub fn clear(&mut self) {
        self.queue.clear()
    }

    pub fn pop(&mut self) -> Option<S> {
        self.queue.pop()
    }
}

impl<S> IntoIterator for Sender<S> {
    type Item = S;

    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.queue.into_iter()
    }
}

pub mod prelude {
    pub use super::Payload;
    pub use super::Query;
}
