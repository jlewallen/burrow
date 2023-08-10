use anyhow::Result;
use std::{
    cell::RefCell,
    collections::HashMap,
    fmt::Debug,
    ops::{Deref, Index},
    rc::Rc,
};
use JsonValue;

mod base;
pub use base::*;

pub mod scopes;
pub use scopes::*;

mod entity;
pub use entity::*;

mod entry;
pub use entry::*;

pub mod props;
pub use props::*;

pub mod compare;
pub use compare::{AnyChanges, CompareChanges, CompareError, Modified, Original};

use super::session::*;

pub trait LoadsEntities {
    fn load_entity(&self, lookup: &LookupBy) -> Result<Option<EntityPtr>>;
}

#[derive(Clone)]
pub struct EntityPtr {
    entity: Rc<RefCell<Entity>>,
    lazy: RefCell<EntityRef>,
}

impl EntityPtr {
    pub fn new_blank() -> Result<Self> {
        Ok(Self::new(
            build_entity()
                .with_key(get_my_session()?.new_key())
                .try_into()?,
        ))
    }

    pub fn new(e: Entity) -> Self {
        let brand_new = Rc::new(RefCell::new(e));
        let lazy = EntityRef::new_from_raw(&brand_new);

        Self {
            entity: brand_new,
            lazy: lazy.into(),
        }
    }

    pub fn new_named(class: EntityClass, name: &str, desc: &str) -> Result<Self, DomainError> {
        Ok(Self::new(
            build_entity()
                .class(class)
                .name(name)
                .desc(desc)
                .try_into()?,
        ))
    }

    // TODO Into/From
    pub fn from_value(value: JsonValue) -> Result<Self, DomainError> {
        Ok(Self::new(Entity::from_value(value)?))
    }

    // TODO Into/From
    pub fn to_json_value(&self) -> Result<JsonValue, DomainError> {
        self.entity.borrow().to_json_value()
    }

    pub fn key(&self) -> EntityKey {
        self.lazy.borrow().key().clone()
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

impl From<&EntityPtr> for EntityRef {
    fn from(value: &EntityPtr) -> Self {
        value.lazy.borrow().clone()
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
        write!(f, "{:?}", lazy)
    }
}

pub trait IntoEntry {
    fn to_entry(&self) -> Result<Entry, DomainError>;
}

pub trait IntoEntity {
    fn to_entity(&self) -> Result<EntityPtr, DomainError>;
}

impl IntoEntity for EntityRef {
    fn to_entity(&self) -> Result<EntityPtr, DomainError> {
        match self.entity() {
            Ok(e) => Ok(e.into()),
            Err(e) => Err(e),
        }
    }
}

impl IntoEntry for EntityRef {
    fn to_entry(&self) -> Result<Entry, DomainError> {
        if !self.key().valid() {
            return Err(DomainError::InvalidKey);
        }
        get_my_session()?
            .entry(&LookupBy::Key(self.key()))?
            .ok_or(DomainError::DanglingEntity)
    }
}

impl TryInto<EntityPtr> for &EntityRef {
    type Error = DomainError;

    fn try_into(self) -> std::result::Result<EntityPtr, Self::Error> {
        self.to_entity()
    }
}

impl TryFrom<EntityRef> for Entry {
    type Error = DomainError;

    fn try_from(value: EntityRef) -> Result<Self, Self::Error> {
        get_my_session()?
            .entry(&LookupBy::Key(value.key()))?
            .ok_or(DomainError::DanglingEntity)
    }
}

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
