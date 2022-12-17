use anyhow::Result;
use nanoid::nanoid;
use once_cell::sync::Lazy;
use replies::Observed;
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

use crate::domain::Entry;

use super::{infra::*, Scope};

pub static WORLD_KEY: Lazy<EntityKey> = Lazy::new(|| EntityKey("world".to_string()));

pub static NAME_PROPERTY: &str = "name";

pub static DESC_PROPERTY: &str = "desc";

pub static GID_PROPERTY: &str = "gid";

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

impl Debug for EntityKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("`{}`", self.0))
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

impl From<EntityKey> for String {
    fn from(key: EntityKey) -> Self {
        key.to_string()
    }
}

#[derive(Clone, Eq, PartialEq, Hash, Serialize, Deserialize, Debug)]
pub struct EntityGID(u64);

impl EntityGID {
    pub fn new(i: u64) -> EntityGID {
        EntityGID(i)
    }

    pub fn gid_to_string(&self) -> String {
        format!("{}", self.0)
    }
}

impl Display for EntityGID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<EntityGID> for u64 {
    fn from(gid: EntityGID) -> Self {
        gid.0
    }
}

impl From<&EntityGID> for u64 {
    fn from(gid: &EntityGID) -> Self {
        gid.0
    }
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
    GID(EntityGID),
    Contained(Box<Item>),
}

#[derive(Clone)]
pub struct EntityPtr {
    entity: Rc<RefCell<Entity>>,
    lazy: RefCell<LazyLoadedEntity>,
}

impl EntityPtr {
    pub fn new_blank() -> Self {
        Self::new(Entity::new_blank())
    }

    pub fn new(e: Entity) -> Self {
        let brand_new = Rc::new(RefCell::new(e));
        let lazy = LazyLoadedEntity::new_from_raw(&brand_new);

        Self {
            entity: brand_new,
            lazy: lazy.into(),
        }
    }

    pub fn new_named(name: &str, desc: &str) -> Result<Self> {
        let brand_new = Self::new_blank();

        brand_new.mutate(|e| {
            e.set_name(name)?;
            e.set_desc(desc)
        })?;

        brand_new.modified()?;

        Ok(brand_new)
    }

    pub fn downgrade(&self) -> Weak<RefCell<Entity>> {
        Rc::downgrade(&self.entity)
    }

    pub fn key(&self) -> EntityKey {
        self.lazy.borrow().key.clone()
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

    pub fn mutate<R, T: FnOnce(&mut Entity) -> Result<R>>(&self, mutator: T) -> Result<R> {
        mutator(&mut self.borrow_mut())
    }

    pub fn mutate_chained<R, T: FnOnce(&mut Entity) -> Result<()>>(
        &self,
        mutator: T,
    ) -> Result<&Self> {
        mutator(&mut self.borrow_mut())?;

        Ok(self)
    }

    pub fn mutate_scope<S: Scope, R, T: FnOnce(&mut S) -> Result<R>>(
        &self,
        mutator: T,
    ) -> Result<R> {
        let mut borrowed = self.borrow_mut();
        let mut scope = borrowed.scope_mut::<S>()?;
        let res = mutator(&mut scope)?;
        scope.save()?;
        Ok(res)
    }

    pub fn read_scope<S: Scope, R, T: FnOnce(&S) -> Result<R>>(&self, mutator: T) -> Result<R> {
        let borrowed = self.borrow();
        let scope = borrowed.scope::<S>()?;
        mutator(&scope)
    }
}

impl From<Rc<RefCell<Entity>>> for EntityPtr {
    fn from(ep: Rc<RefCell<Entity>>) -> Self {
        let lazy = LazyLoadedEntity::new_from_raw(&ep);

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

impl Debug for EntityPtr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let lazy = self.lazy.borrow();
        if let Some(gid) = &lazy.gid {
            f.write_fmt(format_args!(
                "Entity(#{}, `{}`, {})",
                &gid, &lazy.name, &lazy.key
            ))
        } else {
            f.write_fmt(format_args!("Entity(`{}`, {})", &lazy.name, &lazy.key))
        }
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
            identity: identity,
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

    /*
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
    */

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

    /*
    fn set_i64_property(&mut self, name: &str, value: i64) -> Result<()> {
        self.map
            .insert(name.to_owned(), Property::new(serde_json::to_value(value)?));

        Ok(())
    }
    */
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
    parent: Option<LazyLoadedEntity>,
    creator: Option<LazyLoadedEntity>,
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

impl Needs<Rc<dyn Infrastructure>> for Entity {
    fn supply(&mut self, infra: &Rc<dyn Infrastructure>) -> Result<()> {
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

    pub fn new_blank() -> Self {
        let session = get_my_session().expect("No session in Entity::new_blank!");
        Self::new_with_key(session.new_key())
    }

    pub fn new_from(template: &Self) -> Result<Self> {
        let mut brand_new = Self::new_blank();

        // TODO Allow scopes to hook into this process. For example
        // elsewhere in this commit I've wondered about how to copy 'kind'
        // into the new item in the situation for separate, so I'd start
        // there. Ultimately I think it'd be nice if we could just pass a
        // map of scopes in with their intended values.

        // TODO Customize clone to always remove GID_PROPERTY
        brand_new.props = template.props.clone();
        brand_new.props.remove_property(GID_PROPERTY)?;
        brand_new.class = template.class.clone();
        brand_new.acls = template.acls.clone();
        brand_new.parent = template.parent.clone();
        brand_new.creator = template.creator.clone();

        Ok(brand_new)
    }

    pub fn set_key(&mut self, key: &EntityKey) -> Result<()> {
        self.key = key.clone();

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

    pub fn gid(&self) -> Option<EntityGID> {
        self.props.u64_property(GID_PROPERTY).map(EntityGID)
    }

    pub fn set_gid(&mut self, gid: EntityGID) -> Result<()> {
        self.props.set_u64_property(GID_PROPERTY, gid.into())
    }

    pub fn set_version(&mut self, version: u64) -> Result<()> {
        self.version.i = version;
        Ok(())
    }

    pub fn desc(&self) -> Option<String> {
        self.props.string_property(DESC_PROPERTY)
    }

    pub fn set_desc(&mut self, value: &str) -> Result<()> {
        let value: serde_json::Value = value.into();
        self.props.set_property(DESC_PROPERTY, value);

        Ok(())
    }

    pub fn has_scope<T: Scope>(&self) -> bool {
        self.scopes.contains_key(<T as Scope>::scope_key())
    }

    pub fn scope_hack<T: Scope>(&self) -> Result<Box<T>> {
        Ok(self.load_scope::<T>()?)
    }

    pub fn maybe_scope<T: Scope>(&self) -> Result<Option<OpenScope<T>>, DomainError> {
        if !self.has_scope::<T>() {
            return Ok(None);
        }
        let scope = self.load_scope::<T>()?;
        Ok(Some(OpenScope::new(scope)))
    }

    pub fn scope<T: Scope>(&self) -> Result<OpenScope<T>, DomainError> {
        let scope = self.load_scope::<T>()?;
        Ok(OpenScope::new(scope))
    }

    pub fn scope_mut<T: Scope>(&mut self) -> Result<OpenScopeMut<T>, DomainError> {
        let scope = self.load_scope::<T>()?;
        Ok(OpenScopeMut::new(self, scope))
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

#[derive(Clone, Serialize, Deserialize, Default)]
pub struct LazyLoadedEntity {
    #[serde(rename = "py/object")]
    py_object: String,
    #[serde(rename = "py/ref")]
    py_ref: String,
    pub key: EntityKey,
    #[serde(rename = "klass")]
    class: String,
    name: String,
    gid: Option<EntityGID>,
    #[serde(skip)]
    entity: Option<Weak<RefCell<Entity>>>,
}

impl LazyLoadedEntity {
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
        let session = get_my_session()?;
        Ok(session.entry(&self.key)?.expect("No Entry for Entity"))
    }
}

impl Debug for LazyLoadedEntity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LazyLoadedEntity")
            .field("key", &self.key)
            .field("name", &self.name)
            .field("gid", &self.gid)
            .finish()
    }
}

impl From<EntityPtr> for LazyLoadedEntity {
    fn from(entity: EntityPtr) -> Self {
        entity.lazy.borrow().clone()
    }
}

impl From<&EntityPtr> for LazyLoadedEntity {
    fn from(entity: &EntityPtr) -> Self {
        entity.lazy.borrow().clone()
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
