use anyhow::Result;

use super::{DomainError, EntityKey, EntityPtr, LookupBy, WORLD_KEY};

pub trait EntityPtrResolver {
    fn recursive_entry(
        &self,
        lookup: &LookupBy,
        depth: usize,
    ) -> Result<Option<EntityPtr>, DomainError>;

    fn entry(&self, lookup: &LookupBy) -> Result<Option<EntityPtr>, DomainError> {
        self.recursive_entry(lookup, 0)
    }

    fn world(&self) -> Result<Option<EntityPtr>, DomainError> {
        self.entry(&LookupBy::Key(&EntityKey::new(WORLD_KEY)))
    }
}

