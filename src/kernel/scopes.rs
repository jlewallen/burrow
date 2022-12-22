use anyhow::Result;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::fmt::Debug;

use super::infra::SessionRef;
use super::model::*;

pub use replies::*;

pub type ReplyResult = anyhow::Result<Box<dyn Reply>>;

pub type EvaluationResult = Result<Box<dyn Action>, EvaluationError>;

pub type ActionArgs = (Entry, Entry, Entry, SessionRef);

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
