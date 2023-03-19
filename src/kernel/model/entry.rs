use anyhow::Result;
use std::rc::{Rc, Weak};
use tracing::trace;

use crate::kernel::{
    get_my_session, ActiveSession, DomainError, EntityGid, EntityKey, EntityPtr, EntityRef,
    LookupBy, Scope,
};

#[derive(Clone)]
pub struct Entry {
    key: EntityKey,
    entity: EntityPtr,
    session: Weak<dyn ActiveSession>,
    debug: Option<String>,
}

impl Entry {
    pub fn new(key: &EntityKey, entity: EntityPtr, session: Weak<dyn ActiveSession>) -> Self {
        let debug = Some(format!("{:?}", entity));

        Self {
            key: key.clone(),
            entity,
            session,
            debug,
        }
    }

    pub fn key(&self) -> &EntityKey {
        &self.key
    }

    pub fn entity(&self) -> Result<&EntityPtr> {
        Ok(&self.entity)
    }

    pub fn name(&self) -> Result<Option<String>> {
        let entity = self.entity()?;
        let entity = entity.borrow();

        Ok(entity.name())
    }

    pub fn desc(&self) -> Result<Option<String>> {
        let entity = self.entity()?;
        let entity = entity.borrow();

        Ok(entity.desc())
    }

    pub fn has_scope<T: Scope>(&self) -> Result<bool> {
        let entity = self.entity()?;
        let entity = entity.borrow();

        Ok(entity.has_scope::<T>())
    }

    pub fn scope<T: Scope>(&self) -> Result<OpenedScope<T>> {
        let entity = self.entity()?;
        let entity = entity.borrow();
        let scope = entity.load_scope::<T>()?;

        Ok(OpenedScope::new(scope))
    }

    pub fn scope_mut<T: Scope>(&self) -> Result<OpenedScopeMut<T>> {
        let entity = self.entity()?;
        let entity = entity.borrow();
        let scope = entity.load_scope::<T>()?;

        Ok(OpenedScopeMut::new(Weak::clone(&self.session), self, scope))
    }

    pub fn maybe_scope<T: Scope>(&self) -> Result<Option<OpenedScope<T>>, DomainError> {
        if !self.has_scope::<T>()? {
            return Ok(None);
        }

        Ok(Some(self.scope::<T>()?))
    }

    pub fn gid(&self) -> Option<EntityGid> {
        self.entity.borrow().gid()
    }

    pub fn debug(&self) -> Option<&String> {
        self.debug.as_ref()
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
        Ok(EntityRef::new_with_entity(entry.entity()?))
    }
}

impl TryFrom<EntityRef> for Option<Entry> {
    type Error = DomainError;

    fn try_from(value: EntityRef) -> Result<Self, Self::Error> {
        let session = get_my_session().expect("No active better session");
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

impl<T: Scope> std::ops::Deref for OpenedScope<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.target.as_ref()
    }
}

pub struct OpenedScopeMut<T: Scope> {
    _session: Weak<dyn ActiveSession>,
    owner: Entry,
    target: Box<T>,
}

impl<T: Scope> OpenedScopeMut<T> {
    pub fn new(session: Weak<dyn ActiveSession>, owner: &Entry, target: Box<T>) -> Self {
        trace!("scope-open {:?}", target);

        Self {
            _session: session,
            owner: owner.clone(),
            target,
        }
    }

    pub fn save(&mut self) -> Result<()> {
        let entity = self.owner.entity()?;
        let mut entity = entity.borrow_mut();

        entity.replace_scope::<T>(&self.target)
    }
}

impl<T: Scope> Drop for OpenedScopeMut<T> {
    fn drop(&mut self) {
        // TODO Check for unsaved changes to this scope and possibly warn the
        // user, this would require them to intentionally discard  any unsaved
        // changes. Not being able to bubble an error up makes doing anything
        // elaborate in here a bad idea.
        trace!("scope-dropped {:?}", self.target);
    }
}

impl<T: Scope> std::ops::Deref for OpenedScopeMut<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.target.as_ref()
    }
}

impl<T: Scope> std::ops::DerefMut for OpenedScopeMut<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.target.as_mut()
    }
}
