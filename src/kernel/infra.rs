use anyhow::Result;
use std::{cell::RefCell, rc::Rc};

use super::{
    Action, ActionArgs, DomainError, Entity, EntityGID, EntityKey, EntityPtr, EntityRef, Item,
    LazyLoadedEntity, Reply,
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

pub trait GeneratesGlobalIdentifiers {
    fn generate_gid(&self) -> Result<i64>;
}

pub trait LoadEntities {
    fn load_entity_by_key(&self, key: &EntityKey) -> Result<Option<EntityPtr>>;

    fn load_entity_by_gid(&self, gid: &EntityGID) -> Result<Option<EntityPtr>>;

    fn load_entity_by_ref(&self, entity_ref: &EntityRef) -> Result<Option<EntityPtr>> {
        self.load_entity_by_key(&entity_ref.key)
    }
}

pub trait Infrastructure: LoadEntities {
    fn ensure_entity(&self, entity_ref: &LazyLoadedEntity) -> Result<LazyLoadedEntity>;

    fn prepare_entity(&self, entity: &mut Entity) -> Result<()>;

    fn ensure_optional_entity(
        &self,
        entity_ref: &Option<LazyLoadedEntity>,
    ) -> Result<Option<LazyLoadedEntity>> {
        match entity_ref {
            Some(e) => Ok(Some(self.ensure_entity(e)?)),
            None => Ok(None),
        }
    }

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

    fn add_entity(&self, entity: &EntityPtr) -> Result<()>;

    fn chain(&self, living: &EntityPtr, action: Box<dyn Action>) -> Result<Box<dyn Reply>>;
}

pub trait Needs<T> {
    fn supply(&mut self, resource: &T) -> Result<()>;
}

pub trait SessionTrait: Infrastructure {}
