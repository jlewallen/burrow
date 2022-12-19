use anyhow::Result;
use std::rc::{Rc, Weak};
use tracing::trace;

use crate::kernel::{
    get_my_session, DomainError, EntityKey, EntityPtr, EntityRef, Infrastructure, Scope,
};

#[derive(Clone)]
pub struct Entry {
    pub key: EntityKey,
    pub session: Weak<dyn Infrastructure>,
}

impl TryFrom<EntityPtr> for Entry {
    type Error = DomainError;

    fn try_from(value: EntityPtr) -> Result<Self, Self::Error> {
        Ok(Self {
            key: value.key(),
            session: Rc::downgrade(&get_my_session()?),
        })
    }
}

impl From<&Entry> for EntityRef {
    fn from(value: &Entry) -> Self {
        let entity = get_my_session()
            .expect("No infra")
            .load_entity_by_key(&value.key)
            .expect("Load failed for From to EntityRef")
            .expect("Missing lazy Entity reference");
        EntityRef::new_with_entity(entity)
    }
}

impl Entry {
    pub fn new_for_session(session: &Rc<dyn Infrastructure>) -> Self {
        Self {
            key: EntityKey::default(),
            session: Rc::downgrade(session),
        }
    }

    pub fn key(&self) -> EntityKey {
        self.key.clone()
    }

    pub fn name(&self) -> Option<String> {
        let entity = match self
            .session
            .upgrade()
            .expect("No infra")
            .load_entity_by_key(&self.key)
            .expect("Temporary load for 'name' failed")
        {
            None => panic!("How did you get an Entry for an unknown Entity?"),
            Some(entity) => entity,
        };
        let entity = entity.borrow();

        entity.name()
    }

    pub fn desc(&self) -> Option<String> {
        let entity = match self
            .session
            .upgrade()
            .expect("No infra")
            .load_entity_by_key(&self.key)
            .expect("Temporary load for 'name' failed")
        {
            None => panic!("How did you get an Entry for an unknown Entity?"),
            Some(entity) => entity,
        };
        let entity = entity.borrow();

        entity.desc()
    }

    pub fn has_scope<T: Scope>(&self) -> Result<bool> {
        let entity = match self
            .session
            .upgrade()
            .expect("No infra")
            .load_entity_by_key(&self.key)?
        {
            None => panic!("How did you get an Entry for an unknown Entity?"),
            Some(entity) => entity,
        };

        let entity = entity.borrow();

        Ok(entity.has_scope::<T>())
    }

    pub fn scope<T: Scope>(&self) -> Result<OpenedScope<T>> {
        let entity = match self
            .session
            .upgrade()
            .expect("No infra")
            .load_entity_by_key(&self.key)?
        {
            None => panic!("How did you get an Entry for an unknown Entity?"),
            Some(entity) => entity,
        };

        let entity = entity.borrow();

        let scope = entity.load_scope::<T>()?;

        Ok(OpenedScope::new(scope))
    }

    pub fn scope_mut<T: Scope>(&self) -> Result<OpenedScopeMut<T>> {
        let entity = match self
            .session
            .upgrade()
            .expect("No infra")
            .load_entity_by_key(&self.key)?
        {
            None => panic!("How did you get an Entry for an unknown Entity?"),
            Some(entity) => entity,
        };

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
}

impl TryFrom<EntityRef> for Option<Entry> {
    type Error = DomainError;

    fn try_from(value: EntityRef) -> Result<Self, Self::Error> {
        let session = get_my_session().expect("No active better session");
        Ok(session.entry(&value.key)?)
    }
}

impl std::fmt::Debug for Entry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Entry").field("key", &self.key).finish()
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
    session: Weak<dyn Infrastructure>,
    owner: Entry,
    target: Box<T>,
}

impl<T: Scope> OpenedScopeMut<T> {
    pub fn new(session: Weak<dyn Infrastructure>, owner: &Entry, target: Box<T>) -> Self {
        trace!("scope-open {:?}", target);

        Self {
            session,
            owner: owner.clone(),
            target,
        }
    }

    pub fn save(&mut self) -> Result<()> {
        let entity = self
            .session
            .upgrade()
            .expect("No infra")
            .load_entity_by_key(&self.owner.key)?
            .unwrap();
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
