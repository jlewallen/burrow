use anyhow::Result;
use nanoid::nanoid;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    collections::HashMap,
    fmt::{Debug, Display},
    ops::{Deref, DerefMut, Index},
    rc::{Rc, Weak},
};
use thiserror::Error;
use tracing::{debug, span, trace, Level};

use super::{infra::*, Scope};

pub static WORLD_KEY: Lazy<EntityKey> = Lazy::new(|| EntityKey("world".to_string()));

pub static NAME_PROPERTY: &str = "name";

pub static DESC_PROPERTY: &str = "desc";

pub static GID_PROPERTY: &str = "gid";

pub type EntityPtr = Rc<RefCell<Entity>>;

#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
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

impl Display for EntityKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub trait DomainEvent: Debug {}

#[derive(Debug)]
pub enum DomainOutcome {
    Ok(Vec<Box<dyn DomainEvent>>),
    Nope,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Item {
    Named(String),
    Route(String),
    // ImplicitlyUnheld(String),
    // ImplicitlyHeld(String),
    // ImplicitlyNavigable(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EntityRef {
    #[serde(rename = "py/object")]
    py_object: String,
    #[serde(rename = "py/ref")]
    py_ref: String,
    pub key: EntityKey,
    #[serde(rename = "klass")]
    class: String,
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Identity {
    #[serde(rename = "py/object")]
    py_object: String,
    private: String,
    public: String,
    signature: Option<String>, // TODO Why does this happen in the model?
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Kind {
    #[serde(rename = "py/object")]
    py_object: String,
    identity: Identity,
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
    i: u32,
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

    fn i64_property(&self, name: &str) -> Option<i64> {
        if let Some(property) = self.property_named(name) {
            match &property.value {
                serde_json::Value::Number(v) => v.as_i64(),
                _ => None,
            }
        } else {
            None
        }
    }

    fn set_property(&mut self, name: &str, value: serde_json::Value) {
        self.map.insert(name.to_string(), Property::new(value));
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Entity {
    #[serde(rename = "py/object")]
    py_object: String,
    pub key: EntityKey,
    version: Version,
    parent: Option<LazyLoadedEntity>,
    creator: Option<LazyLoadedEntity>,
    identity: Identity,
    #[serde(rename = "klass")]
    class: EntityClass,
    acls: Acls,
    props: Props,
    scopes: HashMap<String, serde_json::Value>,

    #[serde(skip)] // Very private
    infra: Option<Weak<dyn Infrastructure>>,
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

impl Needs<std::rc::Rc<dyn Infrastructure>> for Entity {
    fn supply(&mut self, infra: &std::rc::Rc<dyn Infrastructure>) -> Result<()> {
        self.infra = Some(Rc::downgrade(infra));
        self.parent = infra.ensure_optional_entity(&self.parent)?;
        self.creator = infra.ensure_optional_entity(&self.creator)?;
        Ok(())
    }
}

impl Entity {
    pub fn new() -> EntityPtr {
        Rc::new(RefCell::new(Self::default()))
    }

    pub fn name(&self) -> Option<String> {
        self.props.string_property(NAME_PROPERTY)
    }

    pub fn set_name(&mut self, value: &str) -> Result<()> {
        let value: serde_json::Value = value.into();
        self.props.set_property(NAME_PROPERTY, value);

        Ok(())
    }

    pub fn desc(&self) -> Option<String> {
        self.props.string_property(DESC_PROPERTY)
    }

    pub fn gid(&self) -> Option<i64> {
        self.props.i64_property(GID_PROPERTY)
    }

    pub fn has_scope<T: Scope>(&self) -> bool {
        self.scopes.contains_key(<T as Scope>::scope_key())
    }

    pub fn scope_mut<T: Scope>(&mut self) -> Result<OpenScopeMut<T>, DomainError> {
        let scope = self.load_scope::<T>()?;
        Ok(OpenScopeMut::new(self, scope))
    }

    pub fn scope<T: Scope>(&self) -> Result<OpenScope<T>, DomainError> {
        let scope = self.load_scope::<T>()?;
        Ok(OpenScope::new(scope))
    }

    pub fn maybe_scope<T: Scope>(&self) -> Result<Option<OpenScope<T>>, DomainError> {
        if !self.has_scope::<T>() {
            return Ok(None);
        }
        let scope = self.load_scope::<T>()?;
        Ok(Some(OpenScope::new(scope)))
    }

    fn load_scope<T: Scope>(&self) -> Result<Box<T>, DomainError> {
        let scope_key = <T as Scope>::scope_key();

        let _load_scope_span = span!(
            Level::DEBUG,
            "scope",
            key = self.key.key_to_string(),
            scope = scope_key
        )
        .entered();

        if !self.scopes.contains_key(scope_key) {
            return Ok(Box::new(Default::default()));
        }

        // The call to serde_json::from_value requires owned data and we have a
        // reference to somebody else's. Presumuably so that we don't couple the
        // lifetime of the returned object to the lifetime of the data being
        // referenced? What's the right solution here?
        // Should the 'un-parsed' Scope also owned the parsed data?
        let data = &self.scopes[scope_key];
        let owned_value = data.clone();
        let mut scope: Box<T> = serde_json::from_value(owned_value)?;

        let _prepare_span = span!(Level::DEBUG, "prepare").entered();

        if let Some(infra) = &self.infra {
            if let Some(infra) = infra.upgrade() {
                scope.supply(&infra)?;
            } else {
                return Err(DomainError::ExpiredInfrastructure);
            }
        } else {
            return Err(DomainError::NoInfrastructure);
        }

        Ok(scope)
    }

    fn replace_scope<T: Scope>(&mut self, scope: &T) -> Result<()> {
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

        self.scopes.insert(scope_key.to_string(), value);

        Ok(())
    }
}

pub struct OpenScope<T: Scope> {
    target: Box<T>,
}

impl<T: Scope> OpenScope<T> {
    pub fn new(target: Box<T>) -> Self {
        trace!("scope-open {:?}", target);

        Self { target }
    }
}

impl<T: Scope> Deref for OpenScope<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.target.as_ref()
    }
}

pub struct OpenScopeMut<'me, T: Scope> {
    owner: &'me mut Entity,
    target: Box<T>,
}

impl<'me, T: Scope> OpenScopeMut<'me, T> {
    pub fn new(owner: &'me mut Entity, target: Box<T>) -> Self {
        trace!("scope-open {:?}", target);

        Self { owner, target }
    }

    pub fn save(&mut self) -> Result<()> {
        self.owner.replace_scope(self.target.as_ref())?;

        Ok(())
    }
}

impl<'me, T: Scope> Drop for OpenScopeMut<'me, T> {
    fn drop(&mut self) {
        // TODO Check for unsaved changes to this scope and possibly warn the
        // user, this would require them to intentionally discard  any unsaved
        // changes. Not being able to bubble an error up makes doing anything
        // elaborate in here a bad idea.
        trace!("scope-dropped {:?}", self.target);
    }
}

impl<'me, T: Scope> Deref for OpenScopeMut<'me, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.target.as_ref()
    }
}

impl<'me, T: Scope> DerefMut for OpenScopeMut<'me, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.target.as_mut()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LazyLoadedEntity {
    #[serde(rename = "py/object")]
    py_object: String,
    #[serde(rename = "py/ref")]
    py_ref: String,
    pub key: EntityKey,
    #[serde(rename = "klass")]
    class: String,
    name: String,

    #[serde(skip)]
    entity: Option<Weak<RefCell<Entity>>>,
}

impl LazyLoadedEntity {
    pub fn new_with_entity(entity: EntityPtr) -> Self {
        let shared_entity = entity.borrow();
        Self {
            py_object: "model.entity.EntityRef".to_string(), // #python-class
            py_ref: "model.entity.Entity".to_string(),       // #python-class
            key: shared_entity.key.clone(),
            class: shared_entity.class.py_type.clone(),
            name: shared_entity.name().unwrap_or_else(|| "".to_string()),
            entity: Some(Rc::downgrade(&entity)),
        }
    }

    pub fn has_entity(&self) -> bool {
        self.entity.is_some()
    }

    pub fn into_entity(&self) -> Result<EntityPtr, DomainError> {
        match &self.entity {
            Some(e) => e.upgrade().ok_or(DomainError::DanglingEntity),
            None => Err(DomainError::DanglingEntity),
        }
    }
}

impl From<EntityPtr> for LazyLoadedEntity {
    fn from(entity: EntityPtr) -> Self {
        LazyLoadedEntity::new_with_entity(entity)
    }
}

#[derive(Error, Debug)]
pub enum DomainError {
    #[error("no such scope '{:?}' on entity '{:?}'", .0, .1)]
    NoSuchScope(EntityKey, String),
    #[error("parse failed")]
    ParseFailed(#[source] serde_json::Error),
    #[error("dangling entity")]
    DanglingEntity,
    #[error("anyhow error")]
    Anyhow(#[source] anyhow::Error),
    #[error("no infrastructure")]
    NoInfrastructure,
    #[error("expired infrastructure")]
    ExpiredInfrastructure,
    #[error("session closed")]
    SessionClosed,
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
    #[error("parse failed")]
    ParseFailed,
}

impl From<nom::Err<nom::error::Error<&str>>> for EvaluationError {
    fn from(source: nom::Err<nom::error::Error<&str>>) -> EvaluationError {
        match source {
            nom::Err::Incomplete(_) => EvaluationError::ParseFailed,
            nom::Err::Error(_e) => EvaluationError::ParseFailed,
            nom::Err::Failure(_e) => EvaluationError::ParseFailed,
        }
    }
}
