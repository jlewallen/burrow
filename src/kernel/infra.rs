use anyhow::Result;
use std::{cell::RefCell, rc::Rc};

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

pub trait LoadEntities {
    fn load_entity_by_key(&self, key: &EntityKey) -> Result<Option<EntityPtr>>;

    fn load_entity_by_gid(&self, gid: &EntityGID) -> Result<Option<EntityPtr>>;

    fn load_entity_by_ref(&self, entity_ref: &EntityRef) -> Result<Option<EntityPtr>> {
        self.load_entity_by_key(&entity_ref.key)
    }
}

/// I think this will eventually need to return or take a construct that's
/// builder-like so callers can take more control. Things to consider are:
/// 1) Conditional needle visibility.
/// 2) Items containing others.
/// 3) Verb capabilities of the needle.
pub trait FindsItems {
    fn find_item(&self, args: ActionArgs, item: &Item) -> Result<Option<EntityPtr>>;

    fn find_optional_item(
        &self,
        args: ActionArgs,
        item: Option<Item>,
    ) -> Result<Option<EntityPtr>> {
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

    fn add_entity(&self, entity: &EntityPtr) -> Result<()>;

    fn add_entities(&self, entities: &Vec<&EntityPtr>) -> Result<()> {
        for entity in entities {
            self.add_entity(entity)?;
        }
        Ok(())
    }

    fn new_key(&self) -> EntityKey;

    fn new_identity(&self) -> Identity;

    fn raise(&self, event: Box<dyn DomainEvent>) -> Result<()>;

    fn chain(&self, living: &EntityPtr, action: Box<dyn Action>) -> Result<Box<dyn Reply>>;
}

pub trait Needs<T> {
    fn supply(&mut self, resource: &T) -> Result<()>;
}

pub trait SessionTrait: Infrastructure {}
