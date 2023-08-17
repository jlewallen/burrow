use replies::TaggedJsonError;
use serde::{Deserialize, Serialize};
use std::{
    fmt::{Debug, Display},
    str::FromStr,
};
use thiserror::Error;
use tracing::*;

pub use replies::JsonValue;

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
pub enum Item {
    Area,
    Myself,
    Named(String),
    Route(String),
    Gid(EntityGid),
    Contained(Box<Item>),
    Held(Box<Item>),
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AclRule {
    keys: Vec<String>,
    perm: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Acls {
    rules: Vec<AclRule>,
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
