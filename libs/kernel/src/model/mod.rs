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
mod builder;
mod entity;
mod entity_ref;
mod entry;

pub mod compare;
pub mod props;
pub mod scopes;

use compare::{AnyChanges, CompareChanges, CompareError, Modified, Original};

pub use base::*;
pub use builder::*;
pub use entity::*;
pub use entity_ref::*;
pub use entry::*;
pub use props::*;
pub use scopes::*;

use super::session::*;

#[derive(Clone)]
pub struct EntityPtr(Rc<RefCell<Entity>>);

impl EntityPtr {
    pub fn new(e: Entity) -> Self {
        Self(Rc::new(RefCell::new(e)))
    }
}

impl Deref for EntityPtr {
    type Target = RefCell<Entity>;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

pub trait IntoEntry {
    fn to_entry(&self) -> Result<Entry, DomainError>;
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
