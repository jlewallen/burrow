use super::*;
use std::{cell::RefCell, rc::Rc};

pub trait Infrastructure: Debug + LoadEntities {
    fn ensure_entity(&self, entity_ref: &DynamicEntityRef) -> Result<DynamicEntityRef>;

    fn find_item(&self, args: ActionArgs, item: &Item) -> Result<Option<Rc<RefCell<Entity>>>>;

    fn find_optional_item(
        &self,
        args: ActionArgs,
        item: Option<Item>,
    ) -> Result<Option<Rc<RefCell<Entity>>>> {
        if let Some(item) = item {
            self.find_item(args, &item)
        } else {
            Ok(None)
        }
    }

    fn ensure_optional_entity(
        &self,
        entity_ref: &Option<DynamicEntityRef>,
    ) -> Result<Option<DynamicEntityRef>> {
        match entity_ref {
            Some(e) => Ok(Some(self.ensure_entity(e)?)),
            None => Ok(None),
        }
    }
}

pub trait PrepareWithInfrastructure {
    fn prepare_with(&mut self, infra: &Weak<dyn Infrastructure>) -> Result<()>;
}
