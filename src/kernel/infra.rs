use super::*;

pub trait Infrastructure: Debug + LoadEntities {
    fn ensure_entity(&self, entity_ref: &DynamicEntityRef) -> Result<DynamicEntityRef>;

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
