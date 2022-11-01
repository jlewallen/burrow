use crate::kernel::{DomainOutcome, EntityPtr};
use anyhow::Result;
use std::rc::Rc;

use super::{
    carrying::model::{Containing, Location},
    moving::model::{Occupyable, Occupying},
};

pub fn move_between(from: EntityPtr, to: EntityPtr, item: EntityPtr) -> Result<DomainOutcome> {
    let mut from = from.borrow_mut();
    let mut from_container = from.scope_mut::<Containing>()?;

    // TODO Maybe the EntityPtr type becomes a wrapping struct and also knows
    // the EntityKey that it points at.
    // info!("moving {:?}!", item.borrow().key);

    match from_container.stop_carrying(Rc::clone(&item))? {
        DomainOutcome::Ok(events) => {
            if false {
                let mut item = item.borrow_mut();
                let mut item_location = item.scope_mut::<Location>()?;
                // TODO How do avoid this clone?
                item_location.container = Some(to.clone().into());
                item_location.save()?;
            }

            let mut to = to.borrow_mut();
            let mut into_container = to.scope_mut::<Containing>()?;

            into_container.start_carrying(item)?;
            into_container.save()?;
            from_container.save()?;

            Ok(DomainOutcome::Ok(events))
        }
        DomainOutcome::Nope => Ok(DomainOutcome::Nope),
    }
}

pub fn navigate_between(from: EntityPtr, to: EntityPtr, item: EntityPtr) -> Result<DomainOutcome> {
    let mut from = from.borrow_mut();
    let mut from_container = from.scope_mut::<Occupyable>()?;

    match from_container.stop_occupying(Rc::clone(&item))? {
        DomainOutcome::Ok(events) => {
            if false {
                let mut item = item.borrow_mut();
                let mut item_location = item.scope_mut::<Occupying>()?;
                // TODO How do avoid this clone?
                item_location.area = to.clone().into();
                item_location.save()?;
            }

            let mut to = to.borrow_mut();
            let mut into_container = to.scope_mut::<Occupyable>()?;

            into_container.start_occupying(item)?;
            into_container.save()?;
            from_container.save()?;

            Ok(DomainOutcome::Ok(events))
        }
        DomainOutcome::Nope => Ok(DomainOutcome::Nope),
    }
}
