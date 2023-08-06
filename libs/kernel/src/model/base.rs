use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::{
    fmt::{Debug, Display},
    str::FromStr,
};
use thiserror::Error;
use tracing::*;

pub use replies::*;

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

#[derive(Debug, Clone)]
pub enum When {
    Interval(Duration),
    Time(DateTime<Utc>),
}

impl When {
    pub fn to_utc_time(&self) -> std::result::Result<DateTime<Utc>, DomainError> {
        match self {
            When::Interval(duration) => Ok(Utc::now()
                .checked_add_signed(*duration)
                .ok_or_else(|| DomainError::Overflow)?),
            When::Time(time) => Ok(*time),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Audience {
    Nobody,
    Everybody,
    Individuals(Vec<EntityKey>),
    Area(EntityKey),
}

#[derive(Debug)]
pub enum DomainOutcome {
    Ok,
    Nope,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum Item {
    Area,
    Myself,
    Named(String),
    Route(String),
    Gid(EntityGid),
    Contained(Box<Item>),
    Held(Box<Item>),
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize, Default)]
pub struct Identity {
    private: String,
    public: String,
}

impl Identity {
    pub fn new(public: String, private: String) -> Self {
        Self { private, public }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize, Default)]
pub struct Kind {
    identity: Identity,
}

impl Kind {
    pub fn new(identity: Identity) -> Self {
        Self { identity }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EntityClass {
    #[serde(rename = "py/type")]
    pub py_type: String,
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
            py_type: value.to_owned(),
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AclRule {
    keys: Vec<String>,
    perm: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Acls {
    rules: Vec<AclRule>,
}

#[derive(Error, Debug)]
pub enum DomainError {
    #[error("No such scope '{:?}' on entity '{:?}'", .0, .1)]
    NoSuchScope(EntityKey, String),
    #[error("Parse failed")]
    ParseFailed(#[source] serde_json::Error),
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
    EntityNotFound,
    #[error("Impossible")]
    Impossible,
    #[error("Overflow")]
    Overflow,
    #[error("Invalid key")]
    InvalidKey,
}

impl From<serde_json::Error> for DomainError {
    fn from(source: serde_json::Error) -> Self {
        DomainError::ParseFailed(source)
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
    #[error("Anyhow error")]
    Anyhow(#[source] anyhow::Error),
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
        EvaluationError::Anyhow(source)
    }
}
