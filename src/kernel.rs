use anyhow::Result;
use markdown_gen::markdown;
use once_cell::sync::Lazy;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Display;
use std::ops::Index;
use std::rc::Rc;
use std::string::FromUtf8Error;
use thiserror::Error;
use tracing::{debug, span, Level};

pub static WORLD_KEY: Lazy<EntityKey> = Lazy::new(|| "world".to_string());

pub type ActionArgs<'a> = (&'a Entity, &'a Entity, &'a Entity);

pub type Markdown = markdown::Markdown<Vec<u8>>;

pub fn markdown_to_string(md: Markdown) -> Result<String, FromUtf8Error> {
    String::from_utf8(md.into_inner())
}
pub trait DomainEvent {}

pub trait Reply: std::fmt::Debug {
    fn to_markdown(&self) -> Result<Markdown>;
}

pub type EntityKey = String;

pub trait Scope {
    fn scope_key() -> &'static str
    where
        Self: Sized;
}

type BoxedScope<T> = Box<T>;

pub trait Action: std::fmt::Debug {
    fn perform(&self, args: ActionArgs) -> Result<Box<dyn Reply>>;
}

#[derive(Debug)]
pub enum SimpleReply {
    Done,
}

impl Reply for SimpleReply {
    fn to_markdown(&self) -> Result<Markdown> {
        let mut md = Markdown::new(Vec::new());
        md.write("ok!")?;
        Ok(md)
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Item {
    Named(String),
}

#[derive(Error, Debug)]
pub enum DomainError {
    #[error("no such scope: {:?}", .0)]
    NoSuchScope(EntityKey, String),
    #[error("parse failed")]
    ParseFailed(#[source] serde_json::Error),
    #[error("dangling entity")]
    DanglingEntity,
}

#[derive(Error, Debug)]
pub enum EvaluationError {
    #[error("parse failed")]
    ParseFailed,
}

impl Into<EvaluationError> for nom::Err<nom::error::Error<&str>> {
    fn into(self) -> EvaluationError {
        match self {
            nom::Err::Incomplete(_) => EvaluationError::ParseFailed,
            nom::Err::Error(_e) => EvaluationError::ParseFailed,
            nom::Err::Failure(_e) => EvaluationError::ParseFailed,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityRef {
    #[serde(alias = "py/object")]
    pub py_object: String,
    #[serde(alias = "py/ref")]
    pub py_ref: String,
    pub key: EntityKey,
    pub klass: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Identity {
    #[serde(alias = "py/object")]
    py_object: String,
    private: String,
    public: String,
    signature: Option<String>, // TODO Why does this happen?
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Kind {
    #[serde(alias = "py/object")]
    py_object: String,
    identity: Identity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityClass {
    #[serde(alias = "py/type")]
    py_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AclRule {
    #[serde(alias = "py/object")]
    py_object: String,
    keys: Vec<String>,
    perm: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Acls {
    #[serde(alias = "py/object")]
    py_object: String,
    rules: Vec<AclRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Version {
    #[serde(alias = "py/object")]
    py_object: String,
    i: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Property {
    value: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Props {
    map: HashMap<String, Property>,
}

#[derive(Debug)]
pub struct DomainResult<T> {
    pub events: Vec<T>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    #[serde(alias = "py/object")]
    py_object: String,
    pub key: String,
    version: Version,
    parent: Option<EntityRef>,
    creator: Option<EntityRef>,
    identity: Identity,
    #[serde(alias = "klass")]
    class: EntityClass,
    acls: Acls,
    props: Props,
    scopes: HashMap<String, serde_json::Value>,

    #[serde(skip)] // Very private
    session: Option<Rc<dyn DomainInfrastructure>>,
}

impl Display for Entity {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        fmt.debug_struct(&self.class.py_type)
            .field("key", &self.key)
            .field("name", &self.name())
            .finish()
    }
}

impl Entity {
    pub fn set_session(&mut self, session: Rc<dyn DomainInfrastructure>) {
        self.session = Some(session);
    }

    fn property_named(&self, name: &str) -> Option<&Property> {
        if self.props.map.contains_key(name) {
            return Some(self.props.map.index(name));
        }
        None
    }

    pub fn name(&self) -> Option<String> {
        if let Some(property) = self.property_named("name") {
            match &property.value {
                serde_json::Value::String(v) => Some(v.to_string()),
                _ => None,
            }
        } else {
            None
        }
    }

    pub fn has_scope<T: Scope + DeserializeOwned>(&self) -> bool {
        self.scopes.contains_key(<T as Scope>::scope_key())
    }

    pub fn scope<T: Scope + DeserializeOwned + LoadReferences>(
        &self,
    ) -> Result<BoxedScope<T>, DomainError> {
        let scope_key = <T as Scope>::scope_key();

        let _load_scope_span = span!(
            Level::INFO,
            "loading_scope",
            key = self.key,
            scope = scope_key
        )
        .entered();

        if !self.scopes.contains_key(scope_key) {
            return Err(DomainError::NoSuchScope(
                self.key.clone(),
                scope_key.to_string(),
            ));
        }

        let data = &self.scopes[scope_key];

        debug!("parsing");

        // The call to serde_json::from_value requires owned data and we have a
        // reference to somebody else's. Presumuably so that we don't couple the
        // lifetime of the returned object to the lifetime of the data being
        // referenced? What's the right solution here?
        // Should the 'un-parsed' Scope also owned the parsed data?
        let owned_value = data.clone();
        let mut scope: Box<T> = serde_json::from_value(owned_value)?;

        let _prepare_span = span!(Level::INFO, "prepare").entered();

        let _ = scope.load_refs(self.session.as_ref().unwrap().as_ref());

        // Ok, great!
        Ok(scope)
    }
}

impl From<serde_json::Error> for DomainError {
    fn from(source: serde_json::Error) -> Self {
        DomainError::ParseFailed(source)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DynamicEntityRef {
    RefOnly {
        #[serde(alias = "py/object")]
        py_object: String,
        #[serde(alias = "py/ref")]
        py_ref: String,
        key: String,
        klass: String,
        name: String,
    },
    Entity(Box<Entity>),
}

impl DynamicEntityRef {
    pub fn key(&self) -> &String {
        match self {
            DynamicEntityRef::RefOnly {
                py_object: _,
                py_ref: _,
                key,
                klass: _,
                name: _,
            } => key,
            DynamicEntityRef::Entity(e) => &e.key,
        }
    }
}

impl TryFrom<DynamicEntityRef> for Box<Entity> {
    type Error = DomainError;

    fn try_from(value: DynamicEntityRef) -> Result<Self, Self::Error> {
        match value {
            DynamicEntityRef::RefOnly {
                py_object: _,
                py_ref: _,
                key: _,
                klass: _,
                name: _,
            } => Err(DomainError::DanglingEntity),
            DynamicEntityRef::Entity(e) => Ok(e),
        }
    }
}

impl From<DynamicEntityRef> for EntityRef {
    fn from(value: DynamicEntityRef) -> Self {
        match value {
            DynamicEntityRef::RefOnly {
                py_object,
                py_ref,
                key,
                klass,
                name,
            } => EntityRef {
                py_object,
                py_ref,
                key,
                klass,
                name,
            },
            DynamicEntityRef::Entity(e) => e.as_ref().into(),
        }
    }
}

pub trait DomainInfrastructure: std::fmt::Debug {
    fn ensure_loaded(&self, entity_ref: &DynamicEntityRef) -> Result<DynamicEntityRef>;
}

pub trait InfrastructureFactory: std::fmt::Debug {
    fn new_infrastructure(&self) -> Result<Rc<dyn DomainInfrastructure>>;
}

pub trait LoadReferences {
    fn load_refs(&mut self, session: &dyn DomainInfrastructure) -> Result<()>;
}

impl From<&Entity> for EntityRef {
    fn from(e: &Entity) -> Self {
        EntityRef {
            py_object: "todo!".to_string(),
            py_ref: "todo!".to_string(),
            key: e.key.to_string(),
            klass: "todo!".to_string(),
            name: e.name().unwrap_or_default(),
        }
    }
}

#[derive(Debug)]
pub struct PersistedEntity {
    pub key: String,
    pub gid: u32,
    pub version: u32,
    pub serialized: String,
}

pub trait PrepareEntityByKey {
    fn prepare_entity_by_key<T: Fn(&mut Entity) -> Result<()>>(
        &self,
        key: &EntityKey,
        prepare: T,
    ) -> Result<&Entity>;
}

pub trait LoadEntityByKey {
    fn load_entity_by_key(&self, key: &EntityKey) -> Result<&Entity>;

    fn load_entity_by_ref(&self, entity_ref: &EntityRef) -> Result<&Entity> {
        self.load_entity_by_key(&entity_ref.key)
    }

    fn load_entities_by_refs(&self, entity_refs: Vec<EntityRef>) -> Result<Vec<&Entity>> {
        entity_refs
            .into_iter()
            .map(|re| -> Result<&Entity> { self.load_entity_by_ref(&re) })
            .collect()
    }

    fn load_entities_by_keys(&self, entity_keys: Vec<EntityKey>) -> Result<Vec<&Entity>> {
        entity_keys
            .into_iter()
            .map(|key| -> Result<&Entity> { self.load_entity_by_key(&key) })
            .collect()
    }
}
