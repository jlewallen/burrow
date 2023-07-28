use anyhow::Result;
use serde_json::Value;
use std::{fmt::Debug, rc::Rc};

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
pub struct Incoming {
    pub key: String,
    pub serialized: Vec<u8>,
}

impl Incoming {
    pub fn new(key: String, serialized: Vec<u8>) -> Self {
        Self { key, serialized }
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
