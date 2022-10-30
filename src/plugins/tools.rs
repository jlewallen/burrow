use crate::kernel::{DomainOutcome, EntityPtr};
use anyhow::Result;
use std::rc::Rc;

use super::carrying::model::Containing;

pub fn move_between(from: EntityPtr, to: EntityPtr, item: EntityPtr) -> Result<DomainOutcome> {
    let mut from = from.borrow_mut();
    let mut from_container = from.scope_mut::<Containing>()?;

    // TODO Maybe the EntityPtr type becomes a wrapping struct and also knows
    // the EntityKey that it points at.
    // info!("moving {:?}!", item.borrow().key);

    match from_container.stop_carrying(Rc::clone(&item))? {
        DomainOutcome::Ok(_) => {
            let mut to = to.borrow_mut();
            let mut into_container = to.scope_mut::<Containing>()?;

            into_container.hold(item)?;
            into_container.save()?;
            from_container.save()?;

            Ok(DomainOutcome::Ok(vec![]))
        }
        DomainOutcome::Nope => Ok(DomainOutcome::Nope),
    }
}
