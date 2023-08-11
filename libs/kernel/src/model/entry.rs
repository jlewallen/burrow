use anyhow::Result;
use serde::{ser::SerializeStruct, Serialize};
use tracing::trace;

use super::{
    CoreProps, DomainError, Entity, EntityKey, EntityPtr, EntityRef, HasScopes, JsonValue,
    LookupBy, Scope, WORLD_KEY,
};
use crate::session::get_my_session;

pub trait EntryResolver {
    fn recursive_entry(
        &self,
        lookup: &LookupBy,
        depth: usize,
    ) -> Result<Option<Entry>, DomainError>;

    fn entry(&self, lookup: &LookupBy) -> Result<Option<Entry>, DomainError> {
        self.recursive_entry(lookup, 0)
    }

    fn world(&self) -> Result<Option<Entry>, DomainError> {
        self.entry(&LookupBy::Key(&EntityKey::new(WORLD_KEY)))
    }
}

#[derive(Clone)]
pub struct Entry {
    key: EntityKey,
    entity: EntityPtr,
    debug: Option<String>,
}

fn make_debug_string(entity: &Entity) -> String {
    let name = entity.name();
    let gid = entity.gid();

    match (name, gid) {
        (Some(name), Some(gid)) => format!("\"{}#{}\"", name, gid),
        (None, None) => panic!("Entity missing name and GID"),
        (None, Some(_)) => panic!("Entity missing name"),
        (Some(_), None) => panic!("Entity missing GID"),
    }
}

impl Entry {
    pub fn new(entity: EntityPtr) -> Self {
        let (key, debug) = {
            let entity = entity.borrow();
            let key = entity.key().clone();
            let debug = Some(make_debug_string(&entity));
            (key, debug)
        };

        Self { key, entity, debug }
    }

    pub fn new_from_entity(entity: Entity) -> Result<Self, DomainError> {
        Ok(Self {
            key: entity.key().clone(),
            entity: EntityPtr::new(entity),
            debug: None,
        })
    }

    pub fn new_from_json(key: EntityKey, value: JsonValue) -> Result<Self, DomainError> {
        Ok(Self {
            key,
            entity: EntityPtr::new(Entity::from_value(value)?),
            debug: None,
        })
    }

    pub fn key(&self) -> &EntityKey {
        &self.key
    }

    pub fn entity(&self) -> &EntityPtr {
        &self.entity
    }

    pub fn entity_ref(&self) -> EntityRef {
        let entity = self.entity.borrow();
        entity.entity_ref()
    }

    pub fn name(&self) -> Result<Option<String>, DomainError> {
        let entity = self.entity();
        let entity = entity.borrow();

        Ok(entity.name())
    }

    pub fn desc(&self) -> Result<Option<String>, DomainError> {
        let entity = self.entity();
        let entity = entity.borrow();

        Ok(entity.desc())
    }

    pub fn has_scope<T: Scope>(&self) -> Result<bool, DomainError> {
        let entity = self.entity();
        let entity = entity.borrow();
        let scopes = entity.scopes();

        Ok(scopes.has_scope::<T>())
    }

    pub fn scope<T: Scope>(&self) -> Result<OpenedScope<T>, DomainError> {
        let entity = self.entity();
        let entity = entity.borrow();
        let scope = entity.scopes().load_scope::<T>()?;

        Ok(OpenedScope::new(scope))
    }

    pub fn scope_mut<T: Scope>(&self) -> Result<OpenedScopeMut<T>, DomainError> {
        let entity = self.entity();
        let entity = entity.borrow();
        let scope = entity.scopes().load_scope::<T>()?;

        Ok(OpenedScopeMut::new(self.entity(), scope))
    }

    pub fn maybe_scope<T: Scope>(&self) -> Result<Option<OpenedScope<T>>, DomainError> {
        if !self.has_scope::<T>()? {
            return Ok(None);
        }

        Ok(Some(self.scope::<T>()?))
    }

    pub fn debug(&self) -> Option<&String> {
        self.debug.as_ref()
    }

    pub fn to_json_value(&self) -> Result<JsonValue, DomainError> {
        self.entity.borrow().to_json_value()
    }
}

impl Serialize for Entry {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let name = self
            .name()
            .map_err(serde::ser::Error::custom)?
            .unwrap_or_else(|| "None".to_owned());
        let mut state = serializer.serialize_struct("Entry", 2)?;
        state.serialize_field("key", &self.key)?;
        state.serialize_field("name", &name)?;
        state.end()
    }
}

impl TryFrom<EntityPtr> for Entry {
    type Error = DomainError;

    fn try_from(value: EntityPtr) -> Result<Self, Self::Error> {
        Ok(Self::new(value))
    }
}

impl TryFrom<&Entry> for EntityRef {
    type Error = DomainError;

    fn try_from(entry: &Entry) -> Result<Self, Self::Error> {
        Ok(EntityRef::new_from_raw(&entry.entity().0))
    }
}

impl TryFrom<EntityRef> for Option<Entry> {
    type Error = DomainError;

    fn try_from(value: EntityRef) -> Result<Self, Self::Error> {
        let session = get_my_session().expect("No active session");
        session.entry(&LookupBy::Key(value.key()))
    }
}

impl std::fmt::Debug for Entry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(debug) = &self.debug {
            f.write_str(debug)
        } else {
            f.debug_struct("Entry").field("key", &self.key).finish()
        }
    }
}

pub struct OpenedScope<T: Scope> {
    target: Box<T>,
}

impl<T: Scope> OpenedScope<T> {
    pub fn new(target: Box<T>) -> Self {
        trace!("scope-open {:?}", target);

        Self { target }
    }
}

impl<T: Scope> AsRef<T> for OpenedScope<T> {
    fn as_ref(&self) -> &T {
        &self.target
    }
}

impl<T: Scope> std::ops::Deref for OpenedScope<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.target
    }
}

pub struct OpenedScopeMut<T: Scope> {
    owner: EntityPtr,
    target: Box<T>,
}

impl<T: Scope> OpenedScopeMut<T> {
    pub fn new(owner: &EntityPtr, target: Box<T>) -> Self {
        trace!("scope-open {:?}", target);

        Self {
            owner: owner.clone(),
            target,
        }
    }

    pub fn save(&mut self) -> Result<(), DomainError> {
        let mut entity = self.owner.borrow_mut();

        entity.scopes_mut().replace_scope::<T>(&self.target)
    }
}

impl<T: Scope> Drop for OpenedScopeMut<T> {
    fn drop(&mut self) {
        // TODO Check for unsaved changes to this scope and possibly warn the
        // user, this would require them to intentionally discard any unsaved
        // changes. Not being able to bubble an error up makes doing anything
        // elaborate in here a bad idea.
        trace!("scope-dropped {:?}", self.target);
    }
}

impl<T: Scope> std::ops::Deref for OpenedScopeMut<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.target
    }
}

impl<T: Scope> std::ops::DerefMut for OpenedScopeMut<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.target
    }
}
