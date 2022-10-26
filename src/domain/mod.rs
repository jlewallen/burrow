mod eval;
mod internal;

pub mod session;
pub use session::*;

use crate::kernel::Infrastructure;
use crate::{
    domain::internal::{DomainInfrastructure, EntityMap},
    storage::sqlite::SqliteStorage,
};
use anyhow::Result;
use std::rc::Rc;

pub fn new_infra() -> Result<Rc<dyn Infrastructure>> {
    let storage = SqliteStorage::new(":memory:")?;
    let entity_map = EntityMap::new();
    Ok(DomainInfrastructure::new(storage, entity_map))
}
