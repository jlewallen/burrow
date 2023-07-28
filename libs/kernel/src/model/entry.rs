use anyhow::{anyhow, Result};
use serde::{ser::SerializeStruct, Serialize};
use std::rc::{Rc, Weak};
use tracing::trace;

use crate::{
    get_my_session, model::Scope, ActiveSession, CoreProps, DomainError, EntityKey, EntityPtr,
    EntityRef, HasScopes, LookupBy,
};

#[derive(Clone)]
pub struct Entry {
    key: EntityKey,
    entity: EntityPtr,
    session: Option<Weak<dyn ActiveSession>>,
    debug: Option<String>,
}

impl Entry {
    pub fn new(key: &EntityKey, entity: EntityPtr, session: Weak<dyn ActiveSession>) -> Self {
        let debug = Some(format!("{:?}", entity));

        Self {
            key: key.clone(),
            entity,
            session: Some(session),
            debug,
        }
    }

    pub fn new_from_json(key: EntityKey, value: serde_json::Value) -> Result<Self, DomainError> {
        Ok(Self {
            key,
            entity: EntityPtr::from_value(value)?,
            session: None,
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
        EntityRef {
            key: self.key.clone(),
            class: entity.class().to_owned(),
            name: entity.name(),
            gid: entity.gid(),
            entity: None,
        }
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
        let scopes = entity.into_scopes();

        Ok(scopes.has_scope::<T>())
    }

    pub fn scope<T: Scope>(&self) -> Result<OpenedScope<T>, DomainError> {
        let entity = self.entity();
        let entity = entity.borrow();
        let scope = entity.into_scopes().load_scope::<T>()?;

        Ok(OpenedScope::new(scope))
    }

    pub fn scope_mut<T: Scope>(&self) -> Result<OpenedScopeMut<T>, DomainError> {
        let entity = self.entity();
        let entity = entity.borrow();
        let scope = entity.into_scopes().load_scope::<T>()?;

        Ok(OpenedScopeMut::new(
            Weak::clone(
                self.session
                    .as_ref()
                    .ok_or_else(|| anyhow!("No session in Entry::scope_mut"))?,
            ),
            self.entity(),
            scope,
        ))
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
}

impl Serialize for Entry {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let name = self
            .name()
            .map_err(|e| serde::ser::Error::custom(e))?
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
        Ok(Self::new(
            &value.key(),
            value,
            Rc::downgrade(&get_my_session()?),
        ))
    }
}

impl TryFrom<&Entry> for EntityRef {
    type Error = DomainError;

    fn try_from(entry: &Entry) -> Result<Self, Self::Error> {
        Ok(EntityRef::new_from_raw(&entry.entity().entity))
    }
}

impl TryFrom<EntityRef> for Option<Entry> {
    type Error = DomainError;

    fn try_from(value: EntityRef) -> Result<Self, Self::Error> {
        let session = get_my_session().expect("No active session");
        Ok(session.entry(&LookupBy::Key(&value.key))?)
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
    _session: Weak<dyn ActiveSession>,
    owner: EntityPtr,
    target: Box<T>,
}

impl<T: Scope> OpenedScopeMut<T> {
    pub fn new(session: Weak<dyn ActiveSession>, owner: &EntityPtr, target: Box<T>) -> Self {
        trace!("scope-open {:?}", target);

        Self {
            _session: session,
            owner: owner.clone(),
            target,
        }
    }

    pub fn save(&mut self) -> Result<(), DomainError> {
        let mut entity = self.owner.borrow_mut();

        entity.into_scopes_mut().replace_scope::<T>(&self.target)
    }

    pub fn as_ref(&mut self) -> &mut T {
        &mut self.target
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
