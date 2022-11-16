use anyhow::Result;
use std::rc::Rc;

pub mod build;

mod internal;
pub mod session;
pub use build::*;
pub use session::*;

use crate::kernel::Infrastructure;
use crate::{
    domain::internal::{DomainInfrastructure, EntityMap},
    storage::sqlite::SqliteStorage,
};

pub fn new_infra() -> Result<Rc<dyn Infrastructure>> {
    let storage = SqliteStorage::new(":memory:")?;
    let entity_map = EntityMap::new();
    let global_ids = GlobalIds::new();
    let performer = StandardPerformer::new(None);
    Ok(DomainInfrastructure::new(
        storage, entity_map, performer, global_ids,
    ))
}
