use crate::kernel::{DomainOutcome, EntityPtr};
use anyhow::Result;
use tracing::info;

use super::{
    carrying::model::{Containing, Location},
    moving::model::{Occupyable, Occupying},
};

pub fn move_between(from: EntityPtr, to: EntityPtr, item: EntityPtr) -> Result<DomainOutcome> {
    info!("moving {:?}!", item);

    let mut from = from.borrow_mut();
    let mut from_container = from.scope_mut::<Containing>()?;

    match from_container.stop_carrying(item.clone())? {
        DomainOutcome::Ok(events) => {
            {
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
    info!("navigating item {:?}", item);

    info!("navigating from {:?}", from);

    let mut from = from.borrow_mut();
    let mut from_container = from.scope_mut::<Occupyable>()?;

    match from_container.stop_occupying(item.clone())? {
        DomainOutcome::Ok(events) => {
            info!("navigating {:?}", to);

            if true {
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
