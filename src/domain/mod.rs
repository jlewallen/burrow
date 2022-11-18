mod internal;

pub mod build;
pub mod session;

use anyhow::Result;
use std::rc::Rc;

use self::internal::GlobalIds;
use crate::kernel::Infrastructure;
use crate::{
    domain::internal::{DomainInfrastructure, EntityMap},
    storage::sqlite::SqliteStorage,
};
pub use build::*;
pub use session::*;

pub fn new_infra() -> Result<Rc<dyn Infrastructure>> {
    let storage = SqliteStorage::new(":memory:")?;
    let entity_map = EntityMap::new(GlobalIds::new());
    let performer = StandardPerformer::new(None);
    Ok(DomainInfrastructure::new(storage, entity_map, performer))
}
