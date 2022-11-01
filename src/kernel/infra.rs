use anyhow::Result;
use std::fmt::Debug;

use super::{ActionArgs, EntityPtr, Item, LazyLoadedEntity, LoadEntities};

pub trait Infrastructure: Debug + LoadEntities {
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
}

pub trait Needs<T: Debug> {
    fn supply(&mut self, resource: &T) -> Result<()>;
}
