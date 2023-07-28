use anyhow::Result;
use serde_json::Value;
use std::{fmt::Debug, rc::Rc};

use crate::{Audience, DomainEvent};

use super::{session::SessionRef, Entry, Surroundings};

pub use replies::*;

pub type ReplyResult = anyhow::Result<Effect>;

/// TODO Make generic over SessionRef, Surroundings, and Result.
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

#[derive(Debug)]
#[non_exhaustive]
pub enum Perform {
    Living {
        living: Entry,
        action: Box<dyn Action>, // TODO Consider making this recursive?
    },
    Chain(Box<dyn Action>),
    Effect(Effect),
    Incoming(Incoming),
    Ping(String),
    Raised(Raised),
}

pub trait Performer {
    fn perform(&self, perform: Perform) -> Result<Effect>;
}

#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum Effect {
    Action(Rc<dyn Action>),
    Reply(Rc<dyn Reply>),
    Pong(String),
    Ok,
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
