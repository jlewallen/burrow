use super::infra::*;
use super::model::*;
use super::*;

pub trait Action: std::fmt::Debug {
    fn perform(&self, args: ActionArgs) -> ReplyResult;
}

pub trait Scope: PrepareWithInfrastructure + DeserializeOwned {
    fn scope_key() -> &'static str
    where
        Self: Sized;
}

pub trait PrepareEntities {
    fn prepare_entity_by_key<T: Fn(&mut Entity) -> Result<()>>(
        &self,
        key: &EntityKey,
        prepare: T,
    ) -> Result<&Entity, DomainError>;
}

pub trait LoadEntities {
    fn load_entity_by_key(&self, key: &EntityKey) -> Result<&Entity, DomainError>;

    fn load_entity_by_ref(&self, entity_ref: &EntityRef) -> Result<&Entity, DomainError> {
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
