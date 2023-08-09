use anyhow::Result;
use serde::de::DeserializeOwned;
use std::{fmt::Debug, rc::Rc};

use crate::{Audience, DomainEvent, When};

use super::{session::SessionRef, Entry, Surroundings};

pub use replies::*;

pub type ReplyResult = anyhow::Result<Effect>;

pub trait Action: ToJson + Debug {
    fn is_read_only() -> bool
    where
        Self: Sized;

    fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult;
}

#[derive(Debug, Clone)]
pub struct Raised {
    pub key: String,
    pub audience: Audience,
    pub event: Rc<dyn DomainEvent>,
}

impl Raised {
    pub fn new(audience: Audience, key: String, event: Rc<dyn DomainEvent>) -> Self {
        Self {
            key,
            audience,
            event,
        }
    }

    pub fn has_prefix(&self, prefix: &str) -> bool {
        self.key.starts_with(prefix)
    }
}

#[derive(Debug, Clone)]
pub struct Incoming {
    pub key: String,
    pub value: serde_json::Value,
}

impl Incoming {
    pub fn new(key: String, value: serde_json::Value) -> Self {
        Self { key, value }
    }

    pub fn has_prefix(&self, prefix: &str) -> bool {
        self.key.starts_with(prefix)
    }
}

#[derive(Clone, Debug)]
pub struct Scheduling {
    pub key: String,
    pub when: When,
    pub message: serde_json::Value,
}

#[derive(Clone, Debug)]
pub enum PerformAction {
    Instance(Rc<dyn Action>),
}

#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum Perform {
    Living {
        living: Entry,
        action: PerformAction,
    },
    Surroundings {
        surroundings: Surroundings,
        action: PerformAction,
    },
    Chain(PerformAction),
    Delivery(Incoming),
    Raised(Raised),
    Schedule(Scheduling),
    Ping(TracePath),
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
            Perform::Chain(_) => "Chain",
            Perform::Delivery(_) => "Delivery",
            Perform::Raised(_) => "Raised",
            Perform::Schedule(_) => "Schedule",
            Perform::Ping(_) => "Ping",
        }
    }
}

pub trait Performer {
    fn perform(&self, perform: Perform) -> Result<Effect>;
}

#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum RevertReason {
    Mysterious,
    Deliberate(String),
}

#[derive(Clone, Debug)]
pub enum EffectReply {
    Instance(Rc<dyn Reply>),
}

impl ToJson for EffectReply {
    fn to_tagged_json(&self) -> std::result::Result<TaggedJson, TaggedJsonError> {
        match self {
            EffectReply::Instance(reply) => reply.to_tagged_json(),
        }
    }
}

impl PartialEq for EffectReply {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Instance(l0), Self::Instance(r0)) => {
                // TODO It may be time to just get rid of the Box here all
                // together and return JSON, we don't do anything else with this
                // boxed value.
                l0.to_tagged_json().expect("tagged json error")
                    == r0.to_tagged_json().expect("tagged json error")
            }
        }
    }
}

#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum Effect {
    Ok,
    Prevented,
    Reply(EffectReply),
    Pong(TracePath),
}

impl PartialEq for Effect {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Reply(l0), Self::Reply(r0)) => l0 == r0,
            (Self::Pong(l0), Self::Pong(r0)) => l0 == r0,
            _ => core::mem::discriminant(self) == core::mem::discriminant(other),
        }
    }
}

#[derive(Clone, Default, PartialEq)]
pub struct TracePath(Vec<String>);

impl TracePath {
    pub fn push(self, v: String) -> Self {
        Self(self.0.into_iter().chain([v]).collect())
    }
}

impl Debug for TracePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value: String = self.clone().into();
        f.write_str(&value)
    }
}

impl From<TracePath> for String {
    fn from(value: TracePath) -> Self {
        value.0.join("")
    }
}

impl ToJson for Effect {
    fn to_tagged_json(&self) -> std::result::Result<TaggedJson, TaggedJsonError> {
        // TODO I'll need to work on this, if not to make tests scale.
        match self {
            Effect::Reply(reply) => reply.to_tagged_json(),
            _ => todo!(),
        }
    }
}

impl<T: Reply + 'static> From<T> for Effect {
    fn from(value: T) -> Self {
        Self::Reply(EffectReply::Instance(Rc::new(value)))
    }
}

pub trait JsonAs<D> {
    fn json_as(&self) -> Result<D, TaggedJsonError>;
}

impl<T: Action> JsonAs<T> for Perform {
    fn json_as(&self) -> Result<T, TaggedJsonError> {
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

impl<T: Reply + DeserializeOwned> JsonAs<T> for Effect {
    fn json_as(&self) -> Result<T, TaggedJsonError> {
        match self {
            Effect::Reply(reply) => Ok(serde_json::from_value(
                reply.to_tagged_json()?.into_untagged(),
            )?),
            _ => todo!(),
        }
    }
}
