use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    collections::HashMap,
    fmt::{Debug, Display},
    ops::{Deref, Index},
    rc::{Rc, Weak},
};
use thiserror::Error;
use tracing::*;

use replies::ToJson;

pub mod entry;
pub use entry::*;

pub mod scopes;
pub use scopes::*;

use super::{session::*, Needs, Scope};

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

pub trait LoadsEntities {
    fn load_entity(&self, lookup: &LookupBy) -> Result<Option<EntityPtr>>;
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
            When::Time(time) => Ok(time.clone()),
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

pub trait DomainEvent: ToJson + Debug {}

#[derive(Debug)]
pub enum DomainOutcome {
    Ok,
    Nope,
}

#[derive(Debug, Clone, Serialize, Eq, PartialEq)]
pub enum Item {
    Area,
    Named(String),
    Route(String),
    Gid(EntityGid),
    Contained(Box<Item>),
    Held(Box<Item>),
}

#[derive(Clone)]
pub struct EntityPtr {
    entity: Rc<RefCell<Entity>>,
    lazy: RefCell<EntityRef>,
}

pub fn deserialize_entity(serialized: &str) -> Result<Entity, DomainError> {
    deserialize_entity_from_value(serde_json::from_str(serialized)?)
}

pub fn deserialize_entity_from_value(serialized: serde_json::Value) -> Result<Entity, DomainError> {
    let session = get_my_session().with_context(|| "Session for deserialize")?;
    deserialize_entity_from_value_with_session(serialized, Some(session))
}

pub fn deserialize_entity_from_value_with_session(
    serialized: serde_json::Value,
    session: Option<Rc<dyn ActiveSession>>,
) -> Result<Entity, DomainError> {
    trace!("parsing");
    let mut loaded: Entity = serde_json::from_value(serialized)?;
    if let Some(session) = session {
        trace!("session");
        loaded
            .supply(&session)
            .with_context(|| "Supplying session")?;
    }
    Ok(loaded)
}

impl EntityPtr {
    pub fn new_blank() -> Result<Self, DomainError> {
        Ok(Self::new(Entity::new_blank()?))
    }

    pub fn new_with_props(properties: Properties) -> Result<Self, DomainError> {
        Ok(Self::new(Entity::new_with_props(properties)?))
    }

    pub fn new(e: Entity) -> Self {
        let brand_new = Rc::new(RefCell::new(e));
        let lazy = EntityRef::new_from_raw(&brand_new);

        Self {
            entity: brand_new,
            lazy: lazy.into(),
        }
    }

    pub fn new_named(name: &str, desc: &str) -> Result<Self, DomainError> {
        let mut props = Properties::default();
        props.set_name(name)?;
        props.set_desc(desc)?;

        Self::new_with_props(props)
    }

    pub fn new_from_json(value: serde_json::Value) -> Result<Self, DomainError> {
        Ok(Self::new(deserialize_entity_from_value_with_session(
            value, None,
        )?))
    }

    pub fn key(&self) -> EntityKey {
        self.lazy.borrow().key.clone()
    }

    pub fn to_json_value(&self) -> Result<serde_json::Value, DomainError> {
        Ok(self.entity.borrow().to_json_value()?)
    }
}

impl From<Rc<RefCell<Entity>>> for EntityPtr {
    fn from(ep: Rc<RefCell<Entity>>) -> Self {
        let lazy = EntityRef::new_from_raw(&ep);

        Self {
            entity: Rc::clone(&ep),
            lazy: RefCell::new(lazy),
        }
    }
}

impl From<Entity> for EntityPtr {
    fn from(entity: Entity) -> Self {
        Rc::new(RefCell::new(entity)).into()
    }
}

// This seems cleaner than implementing borrow/borrow_mut ourselves and things
// were gnarly when I tried implementing Borrow<T> myself.
impl Deref for EntityPtr {
    type Target = RefCell<Entity>;

    fn deref(&self) -> &Self::Target {
        self.entity.as_ref()
    }
}

impl Debug for EntityPtr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let lazy = self.lazy.borrow();
        if let Some(gid) = &lazy.gid {
            write!(f, "Entity(#{}, `{}`, {})", &gid, &lazy.name, &lazy.key)
        } else {
            write!(f, "Entity(?, `{}`, {})", &lazy.name, &lazy.key)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Identity {
    private: String,
    public: String,
    signature: Option<String>, // TODO Why does this happen in the model?
}

impl Identity {
    pub fn new(public: String, private: String) -> Self {
        Self {
            private,
            public,
            signature: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Kind {
    identity: Identity,
}

impl Kind {
    pub fn new(identity: Identity) -> Self {
        Self {
            identity,
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EntityClass {
    #[serde(rename = "py/type")]
    py_type: String,
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

#[derive(Clone, Deserialize)]
#[serde(untagged)]
#[non_exhaustive]
pub enum ScopeValue {
    Original(serde_json::Value),
    Intermediate {
        value: serde_json::Value,
        original: Option<Box<ScopeValue>>,
    },
}

impl Serialize for ScopeValue {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            ScopeValue::Original(value) => value.serialize(serializer),
            ScopeValue::Intermediate { value, original: _ } => value.serialize(serializer),
        }
    }
}

/// Central Entity model. Right now, the only thing that is ever modified at
/// this level is `version` and even that could easily be swept into a scope.
/// It's even possible that 'version' is removed, as we need to track the value
/// outside of the Entity itself.  The only other thing that could change is
/// possibly `acls, only that's probably infrequent.  As a rule going forward,
/// these should be considered immutable.
#[derive(Clone, Serialize, Deserialize)]
pub struct Entity {
    key: EntityKey,
    parent: Option<EntityRef>,
    creator: Option<EntityRef>,
    identity: Identity,
    #[serde(rename = "klass")]
    class: EntityClass,
    acls: Acls,
    scopes: HashMap<String, ScopeValue>,
}

impl Display for Entity {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        fmt.debug_struct(&self.class.py_type)
            .field("key", &self.key)
            .field("name", &self.name())
            .field("gid", &self.gid())
            .finish()
    }
}

impl Needs<SessionRef> for Entity {
    fn supply(&mut self, session: &SessionRef) -> Result<()> {
        self.parent = session.ensure_optional_entity(&self.parent)?;
        self.creator = session.ensure_optional_entity(&self.creator)?;
        Ok(())
    }
}

impl Entity {
    pub fn new_blank() -> Result<Self> {
        Ok(Self::new_with_key(get_my_session()?.new_key()))
    }

    pub fn new_with_key(key: EntityKey) -> Self {
        Self {
            key,
            parent: Default::default(),
            creator: Default::default(),
            identity: Default::default(),
            class: Default::default(),
            acls: Default::default(),
            scopes: Default::default(),
        }
    }

    // TODO Allow scopes to hook into this process. For example
    // elsewhere in this commit I've wondered about how to copy 'kind'
    // into the new item in the situation for separate, so I'd start
    // there. Ultimately I think it'd be nice if we could just pass a
    // map of scopes in with their intended values.
    pub fn new_with_props(properties: Properties) -> Result<Self> {
        let mut entity = Self::new_blank()?;
        entity.set_props(properties.props())?;
        Ok(entity)
    }

    pub fn new_from(template: &Self) -> Result<Self> {
        // TODO Customize clone to always remove GID_PROPERTY
        let mut props = template.props();
        props.remove_property(GID_PROPERTY)?;
        let mut entity = Self::new_with_props(props.into())?;

        entity.class = template.class.clone();
        entity.acls = template.acls.clone();
        entity.parent = template.parent.clone();
        entity.creator = template.creator.clone();

        Ok(entity)
    }

    pub fn key(&self) -> &EntityKey {
        &self.key
    }

    pub fn has_scope<T: Scope>(&self) -> bool {
        self.scopes.contains_key(<T as Scope>::scope_key())
    }

    pub fn load_scope<T: Scope>(&self) -> Result<Box<T>, DomainError> {
        let scope_key = <T as Scope>::scope_key();

        let _load_scope_span = span!(
            Level::TRACE,
            "scope",
            key = self.key.key_to_string(),
            scope = scope_key
        )
        .entered();

        if !self.scopes.contains_key(scope_key) {
            return Ok(Box::default());
        }

        // The call to serde_json::from_value requires owned data and we have a
        // reference to somebody else's. Presumuably so that we don't couple the
        // lifetime of the returned object to the lifetime of the data being
        // referenced? What's the right solution here?
        // Should the 'un-parsed' Scope also owned the parsed data?
        let data = &self.scopes[scope_key];
        let owned_value = data.clone();
        let mut scope: Box<T> = match owned_value {
            ScopeValue::Original(v)
            | ScopeValue::Intermediate {
                value: v,
                original: _,
            } => serde_json::from_value(v)?,
        };

        let _prepare_span = span!(Level::TRACE, "prepare").entered();
        let session = get_my_session()?;
        scope.supply(&session)?;

        Ok(scope)
    }

    pub fn replace_scope<T: Scope>(&mut self, scope: &T) -> Result<(), DomainError> {
        let scope_key = <T as Scope>::scope_key();

        let _span = span!(
            Level::TRACE,
            "scope",
            key = self.key.key_to_string(),
            scope = scope_key
        )
        .entered();

        let value = scope.serialize()?;

        debug!("scope-replace");

        // TODO Would love to just take the value.
        let original = match self.scopes.get(scope_key) {
            Some(value) => Some(value.clone().into()),
            None => None.into(),
        };

        let value = ScopeValue::Intermediate { value, original };

        self.scopes.insert(scope_key.to_owned(), value);

        Ok(())
    }

    pub fn to_json_value(&self) -> Result<serde_json::Value, DomainError> {
        Ok(serde_json::to_value(self)?)
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct EntityRef {
    key: EntityKey,
    #[serde(rename = "klass")]
    class: String,
    name: String,
    gid: Option<EntityGid>,
    #[serde(skip)]
    entity: Option<Weak<RefCell<Entity>>>,
}

impl Default for EntityRef {
    fn default() -> Self {
        Self {
            key: EntityKey::blank(),
            class: Default::default(),
            name: Default::default(),
            gid: Default::default(),
            entity: Default::default(),
        }
    }
}

impl EntityRef {
    pub fn new_with_entity(entity: &EntityPtr) -> Self {
        Self::new_from_raw(&entity.entity)
    }

    fn new_from_raw(entity: &Rc<RefCell<Entity>>) -> Self {
        let shared_entity = entity.borrow();
        Self {
            key: shared_entity.key.clone(),
            class: shared_entity.class.py_type.clone(),
            name: shared_entity.name().unwrap_or_default(),
            gid: shared_entity.gid(),
            entity: Some(Rc::downgrade(entity)),
        }
    }

    pub fn key(&self) -> &EntityKey {
        &self.key
    }

    pub fn has_entity(&self) -> bool {
        self.entity.is_some()
    }

    pub fn into_entity(&self) -> Result<EntityPtr, DomainError> {
        match &self.entity {
            Some(e) => match e.upgrade() {
                Some(e) => Ok(e.into()),
                None => Err(DomainError::DanglingEntity),
            },
            None => Err(DomainError::DanglingEntity),
        }
    }

    pub fn into_entry(&self) -> Result<Entry, DomainError> {
        get_my_session()?
            .entry(&LookupBy::Key(&self.key))?
            .ok_or(DomainError::DanglingEntity)
    }
}

impl Debug for EntityRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EntityRef")
            .field("key", &self.key)
            .field("name", &self.name)
            .field("gid", &self.gid)
            .finish()
    }
}

impl From<&EntityPtr> for EntityRef {
    fn from(entity: &EntityPtr) -> Self {
        entity.lazy.borrow().clone()
    }
}

impl TryFrom<EntityRef> for Entry {
    type Error = DomainError;

    fn try_from(value: EntityRef) -> Result<Self, Self::Error> {
        get_my_session()?
            .entry(&LookupBy::Key(&value.key))?
            .ok_or(DomainError::DanglingEntity)
    }
}

#[derive(Error, Debug)]
pub enum DomainError {
    #[error("No such scope '{:?}' on entity '{:?}'", .0, .1)]
    NoSuchScope(EntityKey, String),
    #[error("Parse failed")]
    ParseFailed(#[source] serde_json::Error),
    #[error("Dangling entity")]
    DanglingEntity,
    #[error("Anyhow error")]
    Anyhow(#[source] anyhow::Error),
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
