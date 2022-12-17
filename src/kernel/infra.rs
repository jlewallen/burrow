use anyhow::Result;
use std::{cell::RefCell, rc::Rc};

use crate::domain::Entry;

use super::{
    Action, ActionArgs, DomainError, DomainEvent, EntityGID, EntityKey, EntityPtr, EntityRef,
    Identity, Item, LazyLoadedEntity, Reply,
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

pub trait Infrastructure {
    fn load_entity_by_key(&self, key: &EntityKey) -> Result<Option<EntityPtr>>;

    fn load_entity_by_gid(&self, gid: &EntityGID) -> Result<Option<EntityPtr>>;

    fn load_entity_by_ref(&self, entity_ref: &EntityRef) -> Result<Option<EntityPtr>> {
        self.load_entity_by_key(&entity_ref.key)
    }

    fn entry(&self, key: &EntityKey) -> Result<Option<Entry>>;

    /// I think this will eventually need to return or take a construct that's
    /// builder-like so callers can take more control. Things to consider are:
    /// 1) Conditional needle visibility.
    /// 2) Items containing others.
    /// 3) Verb capabilities of the needle.
    fn find_item(&self, args: ActionArgs, item: &Item) -> Result<Option<Entry>>;

    fn find_optional_item(&self, args: ActionArgs, item: Option<Item>) -> Result<Option<Entry>> {
        if let Some(item) = item {
            self.find_item(args, &item)
        } else {
            Ok(None)
        }
    }
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
