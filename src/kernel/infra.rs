use super::{Action, ActionArgs, Entity, EntityPtr, Item, LazyLoadedEntity, LoadEntities, Reply};
use anyhow::Result;

pub trait GeneratesGlobalIdentifiers {
    fn generate_gid(&self) -> Result<i64>;
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
