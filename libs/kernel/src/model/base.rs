use replies::TaggedJsonError;
use serde::{Deserialize, Serialize};
use std::{
    fmt::{Debug, Display},
    str::FromStr,
};
use thiserror::Error;
use tracing::*;

pub use burrow_bon::prelude::Acls;
pub use replies::JsonValue;

use super::EntityPtr;

pub static WORLD_KEY: &str = "world";

pub static NAME_PROPERTY: &str = "name";

pub static DESC_PROPERTY: &str = "desc";

pub static GID_PROPERTY: &str = "gid";

pub static DESTROYED_PROPERTY: &str = "destroyed";

#[derive(Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct EntityKey(String);

impl EntityKey {
    pub fn blank() -> EntityKey {
        EntityKey("".to_string())
    }

    pub fn new(s: &str) -> EntityKey {
        EntityKey(s.to_string())
    }

    pub fn from_string(s: String) -> EntityKey {
        EntityKey(s)
    }

    pub fn key_to_string(&self) -> &str {
        &self.0
    }

    pub fn valid(&self) -> bool {
        !self.0.is_empty()
    }
}

impl Debug for EntityKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "`{}`", self.0)
    }
}

impl Display for EntityKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<EntityKey> for String {
    fn from(key: EntityKey) -> Self {
        key.to_string()
    }
}

impl From<&str> for EntityKey {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

#[derive(Clone, Eq, PartialEq, Hash, Serialize, Deserialize, Debug)]
pub struct EntityGid(u64);

impl EntityGid {
    pub fn new(i: u64) -> EntityGid {
        EntityGid(i)
    }

    pub fn gid_to_string(&self) -> String {
        format!("{}", self.0)
    }

    pub fn next(&self) -> EntityGid {
        EntityGid(self.0 + 1)
    }
}

impl Display for EntityGid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<EntityGid> for u64 {
    fn from(gid: EntityGid) -> Self {
        gid.0
    }
}

impl From<&EntityGid> for u64 {
    fn from(gid: &EntityGid) -> Self {
        gid.0
    }
}

#[derive(Debug)]
pub enum LookupBy<'a> {
    Key(&'a EntityKey),
    Gid(&'a EntityGid),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Audience {
    Nobody,
    Everybody,
    Individuals(Vec<EntityKey>),
    Area(EntityKey),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Quantity {
    Whole(i64),
    Fractional(f64),
}
impl Quantity {
    pub fn as_f32(&self) -> f32 {
        match self {
            Quantity::Whole(v) => *v as f32,
            Quantity::Fractional(v) => *v as f32,
        }
    }
}

impl PartialOrd for Quantity {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (Quantity::Whole(a), Quantity::Whole(b)) => a.partial_cmp(b),
            (Quantity::Whole(a), Quantity::Fractional(b)) => (*a as f64).partial_cmp(b),
            (Quantity::Fractional(a), Quantity::Whole(b)) => a.partial_cmp(&(*b as f64)),
            (Quantity::Fractional(a), Quantity::Fractional(b)) => a.partial_cmp(b),
        }
    }
}

impl From<i32> for Quantity {
    fn from(value: i32) -> Self {
        Self::Whole(value as i64)
    }
}

impl From<i64> for Quantity {
    fn from(value: i64) -> Self {
        Self::Whole(value)
    }
}

impl From<f32> for Quantity {
    fn from(value: f32) -> Self {
        Self::Fractional(value as f64)
    }
}

impl From<f64> for Quantity {
    fn from(value: f64) -> Self {
        Self::Fractional(value)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Item {
    Area,
    Myself,
    Named(String),
    Route(String),
    Gid(EntityGid),
    Key(EntityKey),
    Contained(Box<Item>),
    Quantified(Quantity, Box<Item>),
    Held(Box<Item>),
}

#[derive(Debug, Clone)]
pub enum Found {
    One(EntityPtr),
    Quantified(Quantity, EntityPtr),
}

impl From<EntityPtr> for Found {
    fn from(value: EntityPtr) -> Self {
        Self::One(value)
    }
}

impl Found {
    pub fn one(self) -> Result<EntityPtr, DomainError> {
        match self {
            Found::One(one) => Ok(one),
            Found::Quantified(_, _) => todo!(),
        }
    }

    pub fn entity(&self) -> Result<&EntityPtr, DomainError> {
        match self {
            Found::One(e) | Found::Quantified(_, e) => Ok(e),
        }
    }
}

impl TryInto<EntityPtr> for Found {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<EntityPtr, Self::Error> {
        match self {
            Found::One(one) => Ok(one),
            Found::Quantified(_, _) => todo!(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Identity {
    private: String,
    public: String,
}

impl Identity {
    pub fn new(public: String, private: String) -> Self {
        Self { private, public }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Kind {
    identity: Identity,
}

impl Kind {
    pub fn new(identity: Identity) -> Self {
        Self { identity }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct EntityClass {
    pub name: String,
}

impl FromStr for EntityClass {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::new(s))
    }
}

impl EntityClass {
    fn new(value: &str) -> Self {
        Self {
            name: value.to_owned(),
        }
    }

    pub fn world() -> Self {
        Self::new("scopes.WorldClass")
    }

    pub fn area() -> Self {
        Self::new("scopes.AreaClass")
    }

    pub fn living() -> Self {
        Self::new("scopes.LivingClass")
    }

    pub fn exit() -> Self {
        Self::new("scopes.ExitClass")
    }

    pub fn item() -> Self {
        Self::new("scopes.ItemClass")
    }

    pub fn encyclopedia() -> Self {
        Self::new("scopes.EncyclopediaClass")
    }
}

#[derive(Debug)]
pub enum ErrorContext {
    Simple(String),
}

impl ErrorContext {
    pub fn new(v: String) -> Self {
        Self::Simple(v)
    }
}

impl From<String> for ErrorContext {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

#[derive(Error, Debug)]
pub enum DomainError {
    #[error("No such scope '{:?}' on entity '{:?}'", .0, .1)]
    NoSuchScope(EntityKey, String),
    #[error("Parse failed")]
    ParseFailed(#[source] serde_json::Error),
    #[error("Tagged JSON error")]
    TaggedJsonError(#[source] TaggedJsonError),
    #[error("Dangling entity")]
    DanglingEntity,
    #[error(transparent)]
    Anyhow(anyhow::Error),
    #[error("No session")]
    NoSession,
    #[error("Expired session")]
    ExpiredSession,
    #[error("Session closed")]
    SessionClosed,
    #[error("Container required")]
    ContainerRequired,
    #[error("Entity not found")]
    EntityNotFound(ErrorContext),
    #[error("Impossible")]
    Impossible,
    #[error("Overflow")]
    Overflow,
    #[error("Invalid key")]
    InvalidKey,
    #[error("Evaluation error")]
    EvaluationError,
}

impl From<EvaluationError> for DomainError {
    fn from(_value: EvaluationError) -> Self {
        DomainError::EvaluationError
    }
}

impl From<TaggedJsonError> for DomainError {
    fn from(value: TaggedJsonError) -> Self {
        DomainError::TaggedJsonError(value)
    }
}

impl From<serde_json::Error> for DomainError {
    fn from(source: serde_json::Error) -> Self {
        DomainError::ParseFailed(source) // TODO Backtrace?
    }
}

impl From<anyhow::Error> for DomainError {
    fn from(source: anyhow::Error) -> Self {
        DomainError::Anyhow(source)
    }
}

#[derive(Error, Debug)]
pub enum EvaluationError {
    #[error("Parse failed")]
    ParseFailed,
    #[error("Other error")]
    Other(#[source] anyhow::Error),
}

impl From<nom::Err<nom::error::Error<&str>>> for EvaluationError {
    fn from(source: nom::Err<nom::error::Error<&str>>) -> EvaluationError {
        match source {
            nom::Err::Incomplete(_) => EvaluationError::ParseFailed,
            nom::Err::Error(_) => EvaluationError::ParseFailed,
            nom::Err::Failure(_) => EvaluationError::ParseFailed,
        }
    }
}

impl From<anyhow::Error> for EvaluationError {
    fn from(source: anyhow::Error) -> Self {
        EvaluationError::Other(source)
    }
}
