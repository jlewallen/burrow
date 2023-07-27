use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    collections::HashMap,
    fmt::Debug,
    ops::{Deref, Index},
    rc::{Rc, Weak},
};
use tracing::*;

mod base;
pub use base::*;

mod entity;
pub use entity::*;

mod entry;
pub use entry::*;

pub mod scopes;
pub use scopes::*;

use super::{session::*, Needs, Scope};

pub trait LoadsEntities {
    fn load_entity(&self, lookup: &LookupBy) -> Result<Option<EntityPtr>>;
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
            key: shared_entity.key().clone(),
            class: shared_entity.class().to_owned(),
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

impl TryFrom<EntityRef> for Entry {
    type Error = DomainError;

    fn try_from(value: EntityRef) -> Result<Self, Self::Error> {
        get_my_session()?
            .entry(&LookupBy::Key(&value.key))?
            .ok_or(DomainError::DanglingEntity)
    }
}

impl From<&EntityPtr> for EntityRef {
    fn from(entity: &EntityPtr) -> Self {
        entity.lazy.borrow().clone()
    }
}
