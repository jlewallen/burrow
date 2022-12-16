use anyhow::Result;
use tracing::info;

use super::carrying::model::Carryable;
use super::moving::model::Exit;
use super::{
    carrying::model::{Containing, Location},
    moving::model::{Occupyable, Occupying},
};
use crate::kernel::{get_my_session, model::*};
use crate::kernel::{DomainOutcome, EntityPtr};

pub fn is_container(item: &EntityPtr) -> bool {
    item.borrow().has_scope::<Containing>()
}

pub fn move_between(from: &EntityPtr, to: &EntityPtr, item: &EntityPtr) -> Result<DomainOutcome> {
    let mut from = from.borrow_mut();
    let mut from_container = from.scope_mut::<Containing>()?;

    match from_container.stop_carrying(item)? {
        DomainOutcome::Ok => {
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

            Ok(DomainOutcome::Ok)
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
        DomainOutcome::Ok => {
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

            Ok(DomainOutcome::Ok)
        }
        DomainOutcome::Nope => Ok(DomainOutcome::Nope),
    }
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

pub fn area_of(living: &EntityPtr) -> Result<EntityPtr> {
    let from = living.borrow();
    let occupying = from.scope::<Occupying>()?;
    Ok(occupying.area.into_entity()?)
}

pub fn set_container(container: &EntityPtr, items: &Vec<EntityPtr>) -> Result<()> {
    let mut editing = container.borrow_mut();
    let mut containing = editing.scope_mut::<Containing>()?;
    for item in items {
        containing.start_carrying(item)?;
        let mut item = item.borrow_mut();
        let mut location = item.scope_mut::<Location>()?;
        location.container = Some(container.try_into()?);
        location.save()?;
    }
    containing.save()
}

pub fn set_occupying(area: &EntityPtr, living: &Vec<EntityPtr>) -> Result<()> {
    let mut editing = area.borrow_mut();
    let mut occupyable = editing.scope_mut::<Occupyable>()?;
    for item in living {
        occupyable.start_occupying(item)?;
        let mut item = item.borrow_mut();
        let mut occupying = item.scope_mut::<Occupying>()?;
        occupying.area = area.try_into()?;
        occupying.save()?;
    }
    occupyable.save()
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

pub fn leads_to<'a>(route: &'a EntityPtr, area: &'a EntityPtr) -> Result<&'a EntityPtr> {
    let mut building = route.borrow_mut();
    let mut exit = building.scope_mut::<Exit>()?;
    exit.area = area.into();
    Ok(route)
}

pub fn get_occupant_keys(area: &EntityPtr) -> Result<Vec<EntityKey>> {
    let occupyable = area.borrow().scope::<Occupyable>()?;
    Ok(occupyable
        .occupied
        .iter()
        .map(|e| e.key.clone())
        .collect::<Vec<EntityKey>>())
}

pub fn new_entity() -> Result<EntityPtr> {
    let entity = EntityPtr::new_blank();
    get_my_session()?.add_entity(&entity)?;
    Ok(entity)
}

pub fn new_entity_from(template: &EntityPtr) -> Result<EntityPtr> {
    let entity = EntityPtr::new_from(template)?;
    get_my_session()?.add_entity(&entity)?;
    Ok(entity)
}

pub fn set_quantity(entity: &EntityPtr, quantity: f32) -> Result<&EntityPtr> {
    entity.mutate(|e| {
        let mut carryable = e.scope_mut::<Carryable>()?;
        carryable.set_quantity(quantity)?;

        Ok(())
    })?;

    Ok(entity)
}

pub fn separate(entity: EntityPtr, quantity: f32) -> Result<(EntityPtr, EntityPtr)> {
    let separated =
        entity.mutate(|e| Ok(e.scope_mut::<Carryable>()?.separate(&entity, quantity)?))?;

    Ok((entity, separated))
}
