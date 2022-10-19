use std::cell::RefCell;
use std::ops::Deref;
use std::rc::Rc;

use tracing::{info, trace};

use super::infra::*;
use super::*;

pub static WORLD_KEY: Lazy<EntityKey> = Lazy::new(|| EntityKey("world".to_string()));

pub static NAME_PROPERTY: &str = "name";

pub static DESC_PROPERTY: &str = "desc";

pub static GID_PROPERTY: &str = "gid";

pub type EntityPtr = Rc<RefCell<Entity>>;

#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct EntityKey(String);

impl EntityKey {
    pub fn key_to_string(&self) -> &str {
        &self.0
    }
}

impl Display for EntityKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub trait DomainEvent: Debug {}

#[derive(Debug)]
pub struct DomainResult {
    pub events: Vec<Box<dyn DomainEvent>>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Item {
    Named(String),
    // ImplicitlyUnheld(String),
    // ImplicitlyHeld(String),
    // ImplicitlyNavigable(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityRef {
    #[serde(alias = "py/object")]
    py_object: String,
    #[serde(alias = "py/ref")]
    py_ref: String,
    pub key: EntityKey,
    #[serde(alias = "klass")]
    class: String,
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Identity {
    #[serde(alias = "py/object")]
    py_object: String,
    private: String,
    public: String,
    signature: Option<String>, // TODO Why does this happen in the model?
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    #[serde(alias = "py/object")]
    py_object: String,
    pub key: EntityKey,
    version: Version,
    parent: Option<DynamicEntityRef>,
    creator: Option<DynamicEntityRef>,
    identity: Identity,
    #[serde(alias = "klass")]
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

impl PrepareWithInfrastructure for Entity {
    fn prepare_with(&mut self, infra: &Weak<dyn Infrastructure>) -> Result<()> {
        self.infra = Some(Weak::clone(infra));
        let infra = infra.upgrade().ok_or(DomainError::NoInfrastructure)?;
        self.parent = infra.ensure_optional_entity(&self.parent)?;
        self.creator = infra.ensure_optional_entity(&self.creator)?;
        Ok(())
    }
}

impl Entity {
    pub fn name(&self) -> Option<String> {
        self.props.string_property(NAME_PROPERTY)
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

    pub fn open<T: Scope>(&mut self) -> Result<OpenScope<T>, DomainError> {
        let scope = self.scope::<T>()?;
        Ok(OpenScope::new(self, scope))
    }

    pub fn scope<T: Scope>(&self) -> Result<Box<T>, DomainError> {
        let scope_key = <T as Scope>::scope_key();

        let _load_scope_span = span!(
            Level::DEBUG,
            "scope",
            key = self.key.key_to_string(),
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

        trace!("parsing");

        // The call to serde_json::from_value requires owned data and we have a
        // reference to somebody else's. Presumuably so that we don't couple the
        // lifetime of the returned object to the lifetime of the data being
        // referenced? What's the right solution here?
        // Should the 'un-parsed' Scope also owned the parsed data?
        let owned_value = data.clone();
        let mut scope: Box<T> = serde_json::from_value(owned_value)?;

        let _prepare_span = span!(Level::DEBUG, "prepare").entered();

        if let Some(infra) = &self.infra {
            scope.prepare_with(infra)?;
        } else {
            return Err(DomainError::NoInfrastructure);
        }

        // Ok, great!
        Ok(scope)
    }

    pub fn replace_scope<T: Scope>(&mut self, _scope: &T) -> Result<()> {
        let scope_key = <T as Scope>::scope_key();

        let _load_scope_span = span!(
            Level::DEBUG,
            "scope",
            key = self.key.key_to_string(),
            scope = scope_key
        )
        .entered();

        info!("scope-replace");

        Ok(())
    }
}

pub struct OpenScope<'me, T: Scope> {
    owner: &'me mut Entity,
    target: Box<T>,
}

impl<'me, T: Scope> OpenScope<'me, T> {
    pub fn new(owner: &'me mut Entity, target: Box<T>) -> Self {
        trace!("scope-open {:?}", target);

        Self {
            owner: owner,
            target: target,
        }
    }

    pub fn s(&self) -> &T {
        self.target.as_ref()
    }

    pub fn s_mut(&mut self) -> &mut T {
        self.target.as_mut()
    }

    pub fn save(&mut self) -> Result<()> {
        self.owner.replace_scope(self.target.as_ref())?;
        Ok(())
    }
}

impl<'me, T: Scope> Drop for OpenScope<'me, T> {
    fn drop(&mut self) {
        // TODO Check for unsaved changes to this scope and possibly warn the
        // user, this would require them to intentionally discard  any unsaved
        // changes. Not being able to bubble an error up makes doing anything
        // elaborate in here a bad idea.
        trace!("scope-dropped {:?}", self.target);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferencedEntity {
    #[serde(alias = "py/object")]
    py_object: String,
    #[serde(alias = "py/ref")]
    py_ref: String,
    key: EntityKey,
    #[serde(alias = "klass")]
    class: String,
    name: String,
    #[serde(skip)]
    entity: Weak<RefCell<Entity>>,
}

impl ReferencedEntity {
    pub fn new(entity: Rc<RefCell<Entity>>) -> Self {
        let shared_entity = entity.borrow();

        Self {
            py_object: "py/object".to_string(),
            py_ref: "py/ref".to_string(),
            key: shared_entity.key.clone(),
            class: shared_entity.class.py_type.clone(),
            name: shared_entity.name().unwrap_or("".to_string()),
            entity: Rc::downgrade(&entity),
        }
    }
}

impl TryFrom<ReferencedEntity> for Rc<RefCell<Entity>> {
    type Error = DomainError;

    fn try_from(value: ReferencedEntity) -> Result<Self, Self::Error> {
        match value.entity.upgrade() {
            Some(e) => Ok(e),
            None => Err(DomainError::DanglingEntity),
        }
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
        key: EntityKey,
        #[serde(alias = "klass")]
        class: String,
        name: String,
    },
    Entity(ReferencedEntity),
}

impl DynamicEntityRef {
    pub fn key(&self) -> &EntityKey {
        match self {
            DynamicEntityRef::RefOnly {
                py_object: _,
                py_ref: _,
                key,
                class: _,
                name: _,
            } => key,
            DynamicEntityRef::Entity(e) => &e.key,
        }
    }
}

impl From<EntityPtr> for DynamicEntityRef {
    fn from(entity: EntityPtr) -> Self {
        DynamicEntityRef::Entity(ReferencedEntity::new(entity))
    }
}

impl TryFrom<DynamicEntityRef> for Rc<RefCell<Entity>> {
    type Error = DomainError;

    fn try_from(value: DynamicEntityRef) -> Result<Self, Self::Error> {
        match value {
            DynamicEntityRef::RefOnly {
                py_object: _,
                py_ref: _,
                key: _,
                class: _,
                name: _,
            } => Err(DomainError::DanglingEntity),
            DynamicEntityRef::Entity(e) => e.entity.upgrade().ok_or(DomainError::DanglingEntity),
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
                class,
                name,
            } => EntityRef {
                py_object,
                py_ref,
                key,
                class,
                name,
            },
            DynamicEntityRef::Entity(e) => {
                if let Some(e) = e.entity.upgrade() {
                    return e.borrow().deref().into();
                }
                panic!("empty dynamic reference ");
            }
        }
    }
}

impl From<DynamicEntityRef> for EntityKey {
    fn from(value: DynamicEntityRef) -> Self {
        match value {
            DynamicEntityRef::RefOnly {
                py_object: _,
                py_ref: _,
                key,
                class: _,
                name: _,
            } => key.clone(),
            DynamicEntityRef::Entity(e) => e.key.clone(),
        }
    }
}

impl From<&Entity> for EntityRef {
    fn from(e: &Entity) -> Self {
        EntityRef {
            py_object: "todo!".to_string(),
            py_ref: "todo!".to_string(),
            key: e.key.clone(),
            class: "todo!".to_string(),
            name: e.name().unwrap_or_default(),
        }
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
