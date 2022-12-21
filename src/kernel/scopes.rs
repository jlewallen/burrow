use anyhow::Result;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::fmt::Debug;

use super::infra::*;
use super::model::*;
use super::ReplyResult;
use crate::domain::Entry;

pub type EvaluationResult = Result<Box<dyn Action>, EvaluationError>;

pub type ActionArgs = (Entry, Entry, Entry, SessionRef);

pub trait Action: Debug {
    fn perform(&self, args: ActionArgs) -> ReplyResult;

    fn is_read_only() -> bool
    where
        Self: Sized;
}

pub trait Scope: Default + Needs<SessionRef> + DeserializeOwned + Debug {
    fn scope_key() -> &'static str
    where
        Self: Sized;

    fn serialize(&self) -> Result<Value>;
}

pub trait Needs<T> {
    fn supply(&mut self, resource: &T) -> Result<()>;
}
