use crate::kernel::model::*;
use crate::kernel::{DomainOutcome, EntityPtr};
use anyhow::Result;
use tracing::info;

use super::{
    carrying::model::{Containing, Location},
    moving::model::{Occupyable, Occupying},
};

pub fn is_container(item: &EntityPtr) -> bool {
    item.borrow().has_scope::<Containing>()
}

pub fn move_between(from: &EntityPtr, to: &EntityPtr, item: &EntityPtr) -> Result<DomainOutcome> {
    let mut from = from.borrow_mut();
    let mut from_container = from.scope_mut::<Containing>()?;

    match from_container.stop_carrying(&item)? {
        DomainOutcome::Ok(events) => {
            {
                let mut item = item.borrow_mut();
                let mut item_location = item.scope_mut::<Location>()?;
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

pub fn navigate_between(
    from: &EntityPtr,
    to: &EntityPtr,
    item: &EntityPtr,
) -> Result<DomainOutcome> {
    let mut from = from.borrow_mut();
    let mut from_container = from.scope_mut::<Occupyable>()?;

    match from_container.stop_occupying(item.clone())? {
        DomainOutcome::Ok(events) => {
            info!("navigating {:?}", to);

            {
                let mut item = item.borrow_mut();
                let mut item_location = item.scope_mut::<Occupying>()?;
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

pub fn area_of(living: &EntityPtr) -> Result<EntityPtr> {
    let from = living.borrow();
    let occupying = from.scope::<Occupying>()?;
    Ok(occupying.area.into_entity()?)
}

pub fn container_of(item: &EntityPtr) -> Result<EntityPtr> {
    let item = item.borrow();
    let location = item.scope::<Location>()?;
    if let Some(container) = &location.container {
        Ok(container.into_entity()?)
    } else {
        Err(DomainError::ContainerRequired.into())
    }
}

pub fn set_container(item: &EntityPtr, container: &EntityPtr) -> Result<()> {
    let mut item = item.borrow_mut();
    let mut location = item.scope_mut::<Location>()?;
    location.container = Some(container.try_into()?);
    location.save()
}

pub fn set_occupying(item: &EntityPtr, occupying: &EntityPtr) -> Result<()> {
    let mut item = item.borrow_mut();
    let mut location = item.scope_mut::<Occupying>()?;
    location.area = occupying.try_into()?;
    location.save()
}

pub fn contained_by(container: &EntityPtr) -> Result<Vec<EntityPtr>> {
    let mut entities: Vec<EntityPtr> = vec![];
    let container = container.borrow();
    if let Ok(containing) = container.scope::<Containing>() {
        for entity in &containing.holding {
            entities.push(entity.into_entity()?);
        }
    }
    Ok(entities)
}
