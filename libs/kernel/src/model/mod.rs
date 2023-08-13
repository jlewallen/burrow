use anyhow::Result;
use std::{
    cell::RefCell,
    collections::HashMap,
    fmt::Debug,
    ops::{Deref, Index},
    rc::Rc,
};

pub mod base;
pub mod builder;
pub mod compare;
pub mod entity;
pub mod entity_ref;
pub mod props;
pub mod scopes;

pub use base::*;
pub use builder::*;
pub use entity::*;
pub use entity_ref::*;
pub use props::*;
pub use scopes::*;

#[derive(Clone)]
pub struct EntityPtr(Rc<RefCell<Entity>>);

impl EntityPtr {
    pub fn new_from_entity(e: Entity) -> Result<Self> {
        // TODO Remove Result
        Ok(Self(Rc::new(RefCell::new(e))))
    }

    pub fn new(e: Entity) -> Self {
        Self(Rc::new(RefCell::new(e)))
    }

    pub fn key(&self) -> EntityKey {
        self.0.borrow().key().clone()
    }

    pub fn entity_ref(&self) -> EntityRef {
        let entity = self.0.borrow();
        entity.entity_ref()
    }

    pub fn entity(&self) -> &EntityPtr {
        &self
    }

    pub fn name(&self) -> Result<Option<String>, DomainError> {
        let entity = self.0.borrow();
        Ok(entity.name())
    }

    pub fn desc(&self) -> Result<Option<String>, DomainError> {
        let entity = self.0.borrow();

        Ok(entity.desc())
    }

    pub fn to_json_value(&self) -> Result<JsonValue, DomainError> {
        self.0.borrow().to_json_value()
    }
}

impl From<Rc<RefCell<Entity>>> for EntityPtr {
    fn from(value: Rc<RefCell<Entity>>) -> Self {
        Self(value)
    }
}

impl Into<Rc<RefCell<Entity>>> for EntityPtr {
    fn into(self) -> Rc<RefCell<Entity>> {
        self.0
    }
}

impl Deref for EntityPtr {
    type Target = RefCell<Entity>;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

impl std::fmt::Debug for EntityPtr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Entity").field("key", &self.key()).finish()
    }
}

pub trait IntoEntityPtr {
    fn to_entity(&self) -> Result<EntityPtr, DomainError>;
}

impl IntoEntityPtr for EntityRef {
    fn to_entity(&self) -> Result<EntityPtr, DomainError> {
        use super::session::get_my_session;
        if !self.key().valid() {
            return Err(DomainError::InvalidKey);
        }
        get_my_session()?
            .entity(&LookupBy::Key(self.key()))?
            .ok_or(DomainError::DanglingEntity)
    }
}

pub trait EntityPtrResolver {
    fn recursive_entity(
        &self,
        lookup: &LookupBy,
        depth: usize,
    ) -> Result<Option<EntityPtr>, DomainError>;

    fn entity(&self, lookup: &LookupBy) -> Result<Option<EntityPtr>, DomainError> {
        self.recursive_entity(lookup, 0)
    }

    fn world(&self) -> Result<Option<EntityPtr>, DomainError> {
        self.entity(&LookupBy::Key(&EntityKey::new(WORLD_KEY)))
    }
}

use compare::{AnyChanges, CompareChanges, CompareError, Modified, Original};

pub fn any_entity_changes(
    l: AnyChanges<Option<Original>, EntityPtr>,
) -> Result<Option<Modified>, CompareError> {
    use compare::TreeDiff;

    let value_after = {
        let entity = l.after.borrow();

        serde_json::to_value(&*entity)?
    };

    let value_before: JsonValue = if let Some(original) = &l.before {
        match original {
            Original::String(s) => s.parse()?,
            Original::Json(v) => (*v).clone(),
        }
    } else {
        JsonValue::Null
    };

    let diff = TreeDiff {};

    diff.any_changes(AnyChanges {
        before: value_before,
        after: value_after,
    })
}
