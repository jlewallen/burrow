use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{fmt::Debug, rc::Rc};

pub use replies::{HasTag, JsonValue, TaggedJson, TaggedJsonError, ToTaggedJson};

use crate::model::DomainError;
use crate::session::SessionRef;
use crate::{
    model::{Audience, EntityPtr},
    surround::Surroundings,
};

pub type ReplyResult = anyhow::Result<Effect>;

pub trait Action: HasTag + ToTaggedJson + Debug {
    fn is_read_only() -> bool
    where
        Self: Sized;

    fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult;
}

#[derive(Debug, Clone, Serialize)]
pub struct Raised {
    pub audience: Audience,
    pub key: String,
    pub living: Option<EntityPtr>,
    pub event: TaggedJson,
}

impl Raised {
    pub fn new(
        audience: Audience,
        key: String,
        living: Option<EntityPtr>,
        event: TaggedJson,
    ) -> Self {
        Self {
            audience,
            key,
            living,
            event,
        }
    }

    pub fn has_prefix(&self, prefix: &str) -> bool {
        self.key.starts_with(prefix)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Incoming {
    pub key: String,
    pub value: TaggedJson,
}

impl Incoming {
    pub fn new(key: String, value: TaggedJson) -> Self {
        Self { key, value }
    }

    pub fn has_prefix(&self, prefix: &str) -> bool {
        self.key.starts_with(prefix)
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Scheduling {
    pub key: String,
    pub when: DateTime<Utc>,
    pub message: TaggedJson,
}

#[derive(Clone, Debug)]
pub enum PerformAction {
    Instance(Rc<dyn Action>),
    TaggedJson(TaggedJson),
}

impl Serialize for PerformAction {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            PerformAction::Instance(action) => action
                .to_tagged_json()
                .unwrap()
                .into_tagged()
                .serialize(serializer),
            PerformAction::TaggedJson(tagged) => tagged.serialize(serializer),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[non_exhaustive]
pub enum Perform {
    Living {
        living: EntityPtr,
        action: PerformAction,
    },
    Surroundings {
        surroundings: Surroundings,
        action: PerformAction,
    },
    Delivery(Incoming),
    Raised(Raised),
    Schedule(Scheduling),
}

impl Perform {
    pub fn enum_name(&self) -> &str {
        match self {
            Perform::Living {
                living: _,
                action: _,
            } => "Living",
            Perform::Surroundings {
                surroundings: _,
                action: _,
            } => "Surroundings",
            Perform::Delivery(_) => "Delivery",
            Perform::Raised(_) => "Raised",
            Perform::Schedule(_) => "Schedule",
        }
    }
}

pub trait Performer {
    fn perform(&self, perform: Perform) -> Result<Effect, DomainError>;
}

#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum RevertReason {
    Mysterious,
    Deliberate(String),
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub enum EffectReply {
    TaggedJson(TaggedJson),
}

impl From<TaggedJson> for EffectReply {
    fn from(value: TaggedJson) -> Self {
        Self::TaggedJson(value)
    }
}

impl ToTaggedJson for EffectReply {
    fn to_tagged_json(&self) -> std::result::Result<TaggedJson, TaggedJsonError> {
        match self {
            EffectReply::TaggedJson(tagged_json) => Ok(tagged_json.clone()),
        }
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub enum Effect {
    Ok,
    Prevented,
    Reply(EffectReply),
}

impl From<EffectReply> for Effect {
    fn from(value: EffectReply) -> Self {
        Effect::Reply(value)
    }
}

use replies::{
    AreaObservation, EditorReply, EntityObservation, InsideObservation, MarkdownReply, Reply,
    SimpleReply,
};

impl TryFrom<EntityObservation> for Effect {
    type Error = TaggedJsonError;

    fn try_from(value: EntityObservation) -> std::result::Result<Self, Self::Error> {
        Ok(Self::Reply(value.to_tagged_json()?.into()))
    }
}

impl TryFrom<InsideObservation> for Effect {
    type Error = TaggedJsonError;

    fn try_from(value: InsideObservation) -> std::result::Result<Self, Self::Error> {
        Ok(Self::Reply(value.to_tagged_json()?.into()))
    }
}

impl TryFrom<AreaObservation> for Effect {
    type Error = TaggedJsonError;

    fn try_from(value: AreaObservation) -> std::result::Result<Self, Self::Error> {
        Ok(Self::Reply(value.to_tagged_json()?.into()))
    }
}

impl TryFrom<MarkdownReply> for Effect {
    type Error = TaggedJsonError;

    fn try_from(value: MarkdownReply) -> std::result::Result<Self, Self::Error> {
        Ok(Self::Reply(value.to_tagged_json()?.into()))
    }
}

impl TryFrom<EditorReply> for Effect {
    type Error = TaggedJsonError;

    fn try_from(value: EditorReply) -> std::result::Result<Self, Self::Error> {
        Ok(Self::Reply(value.to_tagged_json()?.into()))
    }
}

impl TryFrom<SimpleReply> for Effect {
    type Error = TaggedJsonError;

    fn try_from(value: SimpleReply) -> std::result::Result<Self, Self::Error> {
        Ok(Self::Reply(value.to_tagged_json()?.into()))
    }
}

pub trait JsonAs<D> {
    type Error;

    fn json_as(&self) -> Result<D, Self::Error>;
}

/*
impl<T: Action> JsonAs<T> for Perform {
    type Error = TaggedJsonError;

    fn json_as(&self) -> Result<T, Self::Error> {
        match self {
            Perform::Living {
                living: _,
                action: _,
            } => todo!(),
            Perform::Surroundings {
                surroundings: _,
                action: _,
            } => todo!(),
            _ => todo!(),
        }
    }
}
*/

impl<T: Reply + DeserializeOwned> JsonAs<T> for Effect {
    type Error = TaggedJsonError;

    fn json_as(&self) -> Result<T, Self::Error> {
        match self {
            Effect::Reply(reply) => Ok(serde_json::from_value(
                reply.to_tagged_json()?.into_untagged(),
            )?),
            _ => todo!(),
        }
    }
}
