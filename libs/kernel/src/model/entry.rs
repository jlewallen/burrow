use anyhow::Result;

use super::{DomainError, EntityKey, EntityPtr, LookupBy, WORLD_KEY};

pub trait EntryResolver {
    fn recursive_entry(
        &self,
        lookup: &LookupBy,
        depth: usize,
    ) -> Result<Option<Entry>, DomainError>;

    fn entry(&self, lookup: &LookupBy) -> Result<Option<Entry>, DomainError> {
        self.recursive_entry(lookup, 0)
    }

    fn world(&self) -> Result<Option<Entry>, DomainError> {
        self.entry(&LookupBy::Key(&EntityKey::new(WORLD_KEY)))
    }
}

pub use new::*;

#[allow(dead_code)]
mod new {
    pub use super::*;

    pub type Entry = EntityPtr;
}
