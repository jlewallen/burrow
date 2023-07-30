use anyhow::Result;
use serde::de::DeserializeOwned;
use serde_json::Value;
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
    fn to_tagged_json(&self) -> std::result::Result<Value, serde_json::Error> {
        match self {
            EffectReply::Instance(reply) => reply.to_tagged_json(),
        }
    }
}

#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum Effect {
    Ok,
    Prevented,
    Nothing,
    // Revert(RevertReason),
    Reply(EffectReply),
    Pong(TracePath),
    // This is tempting. Right now we recursively call the performer. I'm not
    // sure this gives us in benefit, but it could come in really handy when we
    // start to dynamically alter behavior in chained actions. Leaving this as
    // it stands until I have a stronger opinion.
    // Chain(PerformAction),
}

#[derive(Clone, Default)]
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
    fn to_tagged_json(&self) -> std::result::Result<Value, serde_json::Error> {
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
    fn json_as(&self) -> Result<D, serde_json::Error>;
}

impl<T: Action> JsonAs<T> for Perform {
    fn json_as(&self) -> Result<T, serde_json::Error> {
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

fn drop_object_tag(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(o) => {
            let mut iter = o.into_iter();
            if let Some((_key, value)) = iter.next() {
                assert!(iter.next().is_none());
                value
            } else {
                panic!("Expected tagged JSON");
            }
        }
        _ => panic!("Expected tagged JSON"),
    }
}

impl<T: Reply + DeserializeOwned> JsonAs<T> for Effect {
    fn json_as(&self) -> Result<T, serde_json::Error> {
        match self {
            Effect::Reply(reply) => {
                serde_json::from_value(drop_object_tag(reply.to_tagged_json()?))
            }
            _ => todo!(),
        }
    }
}
