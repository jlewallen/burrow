use anyhow::Result;
use thiserror::Error;

pub trait Action {
    fn perform(&self) -> Result<()>;
}

#[derive(Error, Debug)]
pub enum EvaluationError {
    #[error("unknown parsing human readable")]
    ParseError,
}

pub type EntityKey = String;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Entity {
    pub key: EntityKey,
}

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EntityRef {
    pub key: EntityKey,
}

pub trait DomainEvent {}

pub trait Scope {}

#[derive(Debug)]
pub struct DomainResult<T> {
    pub events: Vec<T>,
}

pub struct Domain {}

impl Domain {
    pub fn open_session() -> Session {
        return Session {};
    }
}

pub struct Session {}

impl Session {
    pub fn close() {}
}
