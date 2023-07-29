use anyhow::Result;
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
#[non_exhaustive]
pub enum Perform {
    Living {
        living: Entry,
        action: Rc<dyn Action>, // TODO Consider making this recursive?
    },
    Surroundings {
        surroundings: Surroundings,
        action: Rc<dyn Action>, // TODO Consider making this recursive?
    },
    Chain(Rc<dyn Action>),
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
#[non_exhaustive]
pub enum Effect {
    Ok,
    Prevented,
    Nothing,
    // Revert(RevertReason),
    Reply(Rc<dyn Reply>),
    Pong(TracePath),
    // This is tempting. Right now we recursively call the performer. I'm not
    // sure this gives us in benefit, but it could come in really handy when we
    // start to dynamically alter behavior in chained actions. Leaving this as
    // it stands until I have a stronger opinion.
    // Chain(Rc<dyn Action>),
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

impl Into<String> for TracePath {
    fn into(self) -> String {
        self.0.join("")
    }
}

impl ToJson for Effect {
    fn to_json(&self) -> std::result::Result<Value, serde_json::Error> {
        // TODO I'll need to work on this, if not to make tests scale.
        match self {
            Effect::Reply(reply) => reply.to_json(),
            _ => todo!(),
        }
    }
}

impl<T: Reply + 'static> From<T> for Effect {
    fn from(value: T) -> Self {
        Self::Reply(Rc::new(value))
    }
}
