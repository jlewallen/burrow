use anyhow::Result;
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};
use tracing::*;

use super::{
    Action, ActionArgs, DomainError, DomainEvent, EntityGID, EntityKey, EntityPtr, EntityRef,
    Identity, Item, LazyLoadedEntity, Reply, Scope,
};

thread_local! {
    #[allow(unused)]
    static SESSION: RefCell<Option<std::rc::Weak<dyn Infrastructure>>> = RefCell::new(None)
}

pub fn set_my_session(session: Option<&Rc<dyn Infrastructure>>) -> Result<()> {
    SESSION.with(|s| {
        *s.borrow_mut() = match session {
            Some(session) => Some(Rc::downgrade(session)),
            None => None,
        };

        Ok(())
    })
}

pub fn get_my_session() -> Result<Rc<dyn Infrastructure>> {
    SESSION.with(|s| match &*s.borrow() {
        Some(s) => match s.upgrade() {
            Some(s) => Ok(s),
            None => Err(DomainError::ExpiredInfrastructure.into()),
        },
        None => Err(DomainError::NoInfrastructure.into()),
    })
}

pub trait LoadEntities {
    fn load_entity_by_key(&self, key: &EntityKey) -> Result<Option<EntityPtr>>;

    fn load_entity_by_gid(&self, gid: &EntityGID) -> Result<Option<EntityPtr>>;

    fn load_entity_by_ref(&self, entity_ref: &EntityRef) -> Result<Option<EntityPtr>> {
        self.load_entity_by_key(&entity_ref.key)
    }
}

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

impl From<&Entry> for LazyLoadedEntity {
    fn from(value: &Entry) -> Self {
        let entity = get_my_session()
            .expect("No infra")
            .load_entity_by_key(&value.key)
            .expect("Load failed for From to LazyLoadedEntity")
            .expect("Missing lazy Entity reference");
        LazyLoadedEntity::new_with_entity(entity)
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

        let scope = entity.scope_hack::<T>()?;

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

        let scope = entity.scope_hack::<T>()?;

        Ok(OpenedScopeMut::new(Weak::clone(&self.session), self, scope))
    }

    pub fn maybe_scope<T: Scope>(&self) -> Result<Option<OpenedScope<T>>, DomainError> {
        if !self.has_scope::<T>()? {
            return Ok(None);
        }
        Ok(Some(self.scope::<T>()?))
    }
}

impl TryFrom<LazyLoadedEntity> for Option<Entry> {
    type Error = DomainError;

    fn try_from(value: LazyLoadedEntity) -> Result<Self, Self::Error> {
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

/// I think this will eventually need to return or take a construct that's
/// builder-like so callers can take more control. Things to consider are:
/// 1) Conditional needle visibility.
/// 2) Items containing others.
/// 3) Verb capabilities of the needle.
pub trait FindsItems {
    fn entry(&self, key: &EntityKey) -> Result<Option<Entry>>;

    fn find_item(&self, args: ActionArgs, item: &Item) -> Result<Option<Entry>>;

    fn find_optional_item(&self, args: ActionArgs, item: Option<Item>) -> Result<Option<Entry>> {
        if let Some(item) = item {
            self.find_item(args, &item)
        } else {
            Ok(None)
        }
    }
}

pub trait Infrastructure: LoadEntities + FindsItems {
    fn ensure_entity(&self, entity_ref: &LazyLoadedEntity) -> Result<LazyLoadedEntity>;

    fn ensure_optional_entity(
        &self,
        entity_ref: &Option<LazyLoadedEntity>,
    ) -> Result<Option<LazyLoadedEntity>> {
        match entity_ref {
            Some(e) => Ok(Some(self.ensure_entity(e)?)),
            None => Ok(None),
        }
    }

    fn add_entity(&self, entity: &EntityPtr) -> Result<Entry>;

    fn add_entities(&self, entities: &Vec<&EntityPtr>) -> Result<Vec<Entry>> {
        entities
            .iter()
            .map(|e| self.add_entity(e))
            .collect::<Result<Vec<_>>>()
    }

    fn new_key(&self) -> EntityKey;

    fn new_identity(&self) -> Identity;

    fn raise(&self, event: Box<dyn DomainEvent>) -> Result<()>;

    fn chain(&self, living: &Entry, action: Box<dyn Action>) -> Result<Box<dyn Reply>>;
}

pub trait Needs<T> {
    fn supply(&mut self, resource: &T) -> Result<()>;
}

pub trait SessionTrait: Infrastructure {}
