use anyhow::Result;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::{fmt::Debug, rc::Rc};

use super::{session::SessionRef, Entry, Surroundings};

pub use replies::*;

#[derive(Clone, Debug)]
pub enum Effect {
    Reply(Rc<dyn Reply>),
    Action(Rc<dyn Action>),
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

pub type ReplyResult = anyhow::Result<Effect>;

pub trait Action: Debug {
    fn is_read_only() -> bool
    where
        Self: Sized;

    fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult;
}

pub trait Scope: Needs<SessionRef> + DeserializeOwned + Default + Debug {
    fn scope_key() -> &'static str
    where
        Self: Sized;

    fn serialize(&self) -> Result<Value>;
}

pub trait Needs<T> {
    fn supply(&mut self, resource: &T) -> Result<()>;
}

pub trait Performer {
    fn perform(&self, perform: Perform) -> Result<Effect>;
}

#[derive(Debug)]
pub enum Perform {
    Living {
        living: Entry,
        action: Box<dyn Action>, // Consider making this recursive?
    },
    Chain(Box<dyn Action>),
}
