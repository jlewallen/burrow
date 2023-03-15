use anyhow::Result;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::fmt::Debug;

use super::infra::SessionRef;
use super::model::*;

pub use replies::*;

pub type ReplyResult = anyhow::Result<Box<dyn Reply>>;

pub type EvaluationResult = Result<Box<dyn Action>, EvaluationError>;

#[derive(Debug, Clone)]
pub enum Surroundings {
    Living {
        world: Entry,
        living: Entry,
        area: Entry,
    },
}

impl Surroundings {
    pub fn to_discovery_vec(&self) -> Vec<&Entry> {
        match self {
            Surroundings::Living {
                world: _world,
                living,
                area,
            } => vec![living, area],
        }
    }
}

#[derive(Clone)]
pub struct ActionArgs {
    pub surroundings: Surroundings,
    pub session: SessionRef,
}

impl ActionArgs {
    pub fn new(surroundings: Surroundings, session: SessionRef) -> Self {
        Self {
            surroundings,
            session,
        }
    }

    pub fn unpack(&self) -> (Entry, Entry, Entry, SessionRef) {
        match &self.surroundings {
            Surroundings::Living {
                world,
                living,
                area,
            } => (
                world.clone(),
                living.clone(),
                area.clone(),
                self.session.clone(),
            ),
        }
    }
}

pub trait Action: Debug {
    fn is_read_only() -> bool
    where
        Self: Sized;

    fn perform(&self, args: ActionArgs) -> ReplyResult;
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
