use super::infra::*;
use super::model::*;
use super::*;
use serde_json::Value;
use std::{fmt::Debug, rc::Rc};

pub type ActionArgs = (EntityPtr, EntityPtr, EntityPtr, Rc<dyn Infrastructure>);

pub trait Action: Debug {
    fn perform(&self, args: ActionArgs) -> ReplyResult;

    fn is_read_only() -> bool
    where
        Self: Sized;
}

pub trait Scope: Debug + Default + Needs<Rc<dyn Infrastructure>> + DeserializeOwned {
    fn scope_key() -> &'static str
    where
        Self: Sized;

    fn serialize(&self) -> Result<Value>;
}

pub trait PrepareEntities {
    fn prepare_entity_by_key(&self, key: &EntityKey) -> Result<EntityPtr>;
}

pub trait LoadEntities {
    fn load_entity_by_key(&self, key: &EntityKey) -> Result<EntityPtr>;

    fn load_entity_by_ref(&self, entity_ref: &EntityRef) -> Result<EntityPtr> {
        self.load_entity_by_key(&entity_ref.key)
    }

    /*
    fn load_entities_by_refs(
        &self,
        entity_refs: Vec<EntityRef>,
    ) -> Result<Vec<&Entity>, DomainError> {
        entity_refs
            .into_iter()
            .map(|re| -> Result<&Entity, DomainError> { self.load_entity_by_ref(&re) })
            .collect()
    }

    fn load_entities_by_keys(
        &self,
        entity_keys: Vec<EntityKey>,
    ) -> Result<Vec<&Entity>, DomainError> {
        entity_keys
            .into_iter()
            .map(|key| -> Result<&Entity, DomainError> { self.load_entity_by_key(&key) })
            .collect()
    }
    */
}
