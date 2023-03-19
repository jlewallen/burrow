use anyhow::Result;
use nanoid::nanoid;
use once_cell::sync::Lazy;
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

use replies::Observed;

pub mod entry;

pub use entry::*;

use super::{infra::*, Needs, Scope};

pub static WORLD_KEY: Lazy<EntityKey> = Lazy::new(|| EntityKey("world".to_string()));

pub static NAME_PROPERTY: &str = "name";

pub static DESC_PROPERTY: &str = "desc";

pub static GID_PROPERTY: &str = "gid";

pub static DESTROYED_PROPERTY: &str = "destroyed";

#[derive(Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct EntityKey(String);

impl EntityKey {
    pub fn new(s: &str) -> EntityKey {
        EntityKey(s.to_string())
    }

    pub fn key_to_string(&self) -> &str {
        &self.0
    }
}

impl Default for EntityKey {
    fn default() -> Self {
        Self(nanoid!())
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

pub enum Audience {
    Nobody,
    Everybody,
    Individuals(Vec<EntityKey>),
    Area(Entry),
}

pub trait DomainEvent: Debug {
    fn audience(&self) -> Audience;

    fn observe(&self, user: &Entry) -> Result<Box<dyn Observed>>;
}

#[derive(Debug)]
pub enum DomainOutcome {
    Ok,
    Nope,
}

#[derive(Debug, Clone, Serialize, Eq, PartialEq)]
pub enum Item {
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

impl EntityPtr {
    pub fn new_blank() -> Result<Self> {
        Ok(Self::new(Entity::new_blank()?))
    }

    pub fn new(e: Entity) -> Self {
        let brand_new = Rc::new(RefCell::new(e));
        let lazy = EntityRef::new_from_raw(&brand_new);

        Self {
            entity: brand_new,
            lazy: lazy.into(),
        }
    }

    pub fn new_named(name: &str, desc: &str) -> Result<Self> {
        let brand_new = Self::new_blank()?;

        brand_new.mutate(|e| {
            e.set_name(name)?;
            e.set_desc(desc)
        })?;

        brand_new.modified()?;

        Ok(brand_new)
    }

    pub fn key(&self) -> EntityKey {
        self.lazy.borrow().key.clone()
    }

    pub fn set_key(&self, key: &EntityKey) -> Result<()> {
        self.mutate(|e| e.set_key(key))?;
        self.modified()
    }

    pub fn set_name(&self, name: &str) -> Result<()> {
        self.mutate(|e| e.set_name(name))?;
        self.modified()
    }

    pub fn modified(&self) -> Result<()> {
        let entity = self.borrow();
        let mut lazy = self.lazy.borrow_mut();
        if let Some(name) = entity.name() {
            lazy.name = name;
        }
        lazy.gid = entity.gid();
        lazy.key = entity.key.clone();

        Ok(())
    }

    fn mutate<R, T: FnOnce(&mut Entity) -> Result<R>>(&self, mutator: T) -> Result<R> {
        mutator(&mut self.borrow_mut())
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
            write!(f, "Entity(`{}`, {})", &lazy.name, &lazy.key)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Identity {
    #[serde(rename = "py/object")]
    py_object: String,
    private: String,
    public: String,
    signature: Option<String>, // TODO Why does this happen in the model?
}

impl Identity {
    pub fn new(public: String, private: String) -> Self {
        Self {
            py_object: String::default(),
            private,
            public,
            signature: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Kind {
    #[serde(rename = "py/object")]
    py_object: String,
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
    #[serde(rename = "py/object")]
    py_object: String,
    keys: Vec<String>,
    perm: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Acls {
    #[serde(rename = "py/object")]
    py_object: String,
    rules: Vec<AclRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Version {
    #[serde(rename = "py/object")]
    py_object: String,
    i: u64,
}

impl Default for Version {
    fn default() -> Self {
        Self {
            py_object: Default::default(),
            i: 1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Property {
    #[serde(rename = "py/object")]
    py_object: String,
    acls: Acls,
    value: serde_json::Value,
}

impl Property {
    pub fn new(value: serde_json::Value) -> Self {
        Self {
            py_object: "".to_string(),
            acls: Default::default(),
            value,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Props {
    #[serde(rename = "py/object")]
    py_object: String,
    map: HashMap<String, Property>,
}

impl Default for Props {
    fn default() -> Self {
        Self {
            py_object: "model.properties.Common".to_string(), // #python-class
            map: Default::default(),
        }
    }
}

impl Props {
    fn property_named(&self, name: &str) -> Option<&Property> {
        if self.map.contains_key(name) {
            return Some(self.map.index(name));
        }
        None
    }

    fn string_property(&self, name: &str) -> Option<String> {
        if let Some(property) = self.property_named(name) {
            match &property.value {
                serde_json::Value::String(v) => Some(v.to_string()),
                _ => None,
            }
        } else {
            None
        }
    }

    // TODO Make the next few functions.
    fn u64_property(&self, name: &str) -> Option<u64> {
        if let Some(property) = self.property_named(name) {
            match &property.value {
                serde_json::Value::Number(v) => v.as_u64(),
                _ => None,
            }
        } else {
            None
        }
    }

    fn set_property(&mut self, name: &str, value: serde_json::Value) {
        self.map.insert(name.to_string(), Property::new(value));
    }

    fn set_u64_property(&mut self, name: &str, value: u64) -> Result<()> {
        self.map
            .insert(name.to_owned(), Property::new(serde_json::to_value(value)?));

        Ok(())
    }

    fn remove_property(&mut self, name: &str) -> Result<()> {
        self.map.remove(name);

        Ok(())
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ScopeValue {
    Json(serde_json::Value),
}

#[derive(Clone, Serialize, Deserialize, Default)]
pub struct Entity {
    #[serde(rename = "py/object")]
    py_object: String,
    pub key: EntityKey,
    version: Version,
    parent: Option<EntityRef>,
    creator: Option<EntityRef>,
    identity: Identity,
    #[serde(rename = "klass")]
    class: EntityClass,
    acls: Acls,
    props: Props,
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
    fn supply(&mut self, infra: &SessionRef) -> Result<()> {
        self.parent = infra.ensure_optional_entity(&self.parent)?;
        self.creator = infra.ensure_optional_entity(&self.creator)?;
        Ok(())
    }
}

impl Entity {
    pub fn new_with_key(key: EntityKey) -> Self {
        Self {
            key,
            ..Self::default()
        }
    }

    pub fn new_blank() -> Result<Self> {
        Ok(Self::new_with_key(get_my_session()?.new_key()))
    }

    pub fn new_from(template: &Self) -> Result<Self> {
        let mut entity = Self::new_blank()?;

        // TODO Allow scopes to hook into this process. For example
        // elsewhere in this commit I've wondered about how to copy 'kind'
        // into the new item in the situation for separate, so I'd start
        // there. Ultimately I think it'd be nice if we could just pass a
        // map of scopes in with their intended values.

        // TODO Customize clone to always remove GID_PROPERTY
        entity.props = template.props.clone();
        entity.props.remove_property(GID_PROPERTY)?;
        entity.class = template.class.clone();
        entity.acls = template.acls.clone();
        entity.parent = template.parent.clone();
        entity.creator = template.creator.clone();

        Ok(entity)
    }

    pub fn set_key(&mut self, key: &EntityKey) -> Result<()> {
        self.key = key.clone();

        Ok(())
    }

    pub fn set_version(&mut self, version: u64) -> Result<()> {
        self.version.i = version;

        Ok(())
    }

    pub fn name(&self) -> Option<String> {
        self.props.string_property(NAME_PROPERTY)
    }

    pub fn set_name(&mut self, value: &str) -> Result<()> {
        let value: serde_json::Value = value.into();
        self.props.set_property(NAME_PROPERTY, value);

        Ok(())
    }

    pub fn gid(&self) -> Option<EntityGid> {
        self.props.u64_property(GID_PROPERTY).map(EntityGid)
    }

    pub fn set_gid(&mut self, gid: EntityGid) -> Result<()> {
        self.props.set_u64_property(GID_PROPERTY, gid.into())
    }

    pub fn desc(&self) -> Option<String> {
        self.props.string_property(DESC_PROPERTY)
    }

    pub fn set_desc(&mut self, value: &str) -> Result<()> {
        let value: serde_json::Value = value.into();
        self.props.set_property(DESC_PROPERTY, value);

        Ok(())
    }

    pub fn destroy(&mut self) -> Result<()> {
        let value: serde_json::Value = true.into();
        self.props.set_property(DESTROYED_PROPERTY, value);

        Ok(())
    }

    pub fn has_scope<T: Scope>(&self) -> bool {
        self.scopes.contains_key(<T as Scope>::scope_key())
    }

    pub fn load_scope<T: Scope>(&self) -> Result<Box<T>, DomainError> {
        let scope_key = <T as Scope>::scope_key();

        let _load_scope_span = span!(
            Level::DEBUG,
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
            ScopeValue::Json(v) => serde_json::from_value(v)?,
        };

        let _prepare_span = span!(Level::DEBUG, "prepare").entered();
        let session = get_my_session()?; // Thread local session!
        scope.supply(&session)?;

        Ok(scope)
    }

    pub fn replace_scope<T: Scope>(&mut self, scope: &T) -> Result<()> {
        let scope_key = <T as Scope>::scope_key();

        let _span = span!(
            Level::DEBUG,
            "scope",
            key = self.key.key_to_string(),
            scope = scope_key
        )
        .entered();

        let value = scope.serialize()?;

        debug!("scope-replace");

        self.scopes
            .insert(scope_key.to_string(), ScopeValue::Json(value));

        Ok(())
    }
}

#[derive(Clone, Serialize, Deserialize, Default)]
pub struct EntityRef {
    #[serde(rename = "py/object")]
    py_object: String,
    #[serde(rename = "py/ref")]
    py_ref: String,
    pub key: EntityKey,
    #[serde(rename = "klass")]
    class: String,
    name: String,
    gid: Option<EntityGid>,
    #[serde(skip)]
    entity: Option<Weak<RefCell<Entity>>>,
}

impl EntityRef {
    pub fn new_with_entity(entity: EntityPtr) -> Self {
        Self::new_from_raw(&entity.entity)
    }

    fn new_from_raw(entity: &Rc<RefCell<Entity>>) -> Self {
        let shared_entity = entity.borrow();
        Self {
            py_object: "model.entity.EntityRef".to_string(), // #python-class
            py_ref: "model.entity.Entity".to_string(),       // #python-class
            key: shared_entity.key.clone(),
            class: shared_entity.class.py_type.clone(),
            name: shared_entity.name().unwrap_or_default(),
            gid: shared_entity.gid(),
            entity: Some(Rc::downgrade(entity)),
        }
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
            .entry(&self.key)?
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
            .entry(&value.key)?
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
    #[error("No infrastructure")]
    NoInfrastructure,
    #[error("No session")]
    NoSession,
    #[error("Expired infrastructure")]
    ExpiredInfrastructure,
    #[error("Session closed")]
    SessionClosed,
    #[error("Container required")]
    ContainerRequired,
    #[error("Entity not found")]
    EntityNotFound,
    #[error("Impossible")]
    Impossible,
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
