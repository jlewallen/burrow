use anyhow::Result;
use tracing::info;

use super::carrying::model::Carryable;
use super::moving::model::Exit;
use super::{
    carrying::model::{Containing, Location},
    moving::model::{Occupyable, Occupying},
};
use crate::domain::Entry;
use crate::kernel::{get_my_session, model::*};
use crate::kernel::{DomainOutcome, EntityPtr};

pub fn is_container(item: &Entry) -> Result<bool> {
    item.has_scope::<Containing>()
}

pub fn move_between(from: &Entry, to: &Entry, item: &Entry) -> Result<DomainOutcome> {
    info!("moving {:?} {:?} {:?}", item, from, to);

    let mut from = from.scope_mut::<Containing>()?;
    let mut into = to.scope_mut::<Containing>()?;

    match from.stop_carrying(item)? {
        DomainOutcome::Ok => {
            let mut location = item.scope_mut::<Location>()?;
            location.container = Some(to.try_into()?);

            into.start_carrying(item)?;
            from.save()?;
            into.save()?;
            location.save()?;

            Ok(DomainOutcome::Ok)
        }
        DomainOutcome::Nope => Ok(DomainOutcome::Nope),
    }
}

pub fn navigate_between(from: &Entry, to: &Entry, item: &Entry) -> Result<DomainOutcome> {
    info!("navigating {:?}", item);

    let mut from = from.scope_mut::<Occupyable>()?;
    let mut into = to.scope_mut::<Occupyable>()?;

    match from.stop_occupying(item)? {
        DomainOutcome::Ok => {
            let mut location = item.scope_mut::<Occupying>()?;
            location.area = to.try_into()?;

            into.start_occupying(item)?;
            into.save()?;
            from.save()?;
            location.save()?;

            Ok(DomainOutcome::Ok)
        }
        DomainOutcome::Nope => Ok(DomainOutcome::Nope),
    }
}

pub fn container_of(item: &Entry) -> Result<EntityPtr> {
    let location = item.scope::<Location>()?;
    if let Some(container) = &location.container {
        Ok(container.into_entity()?)
    } else {
        Err(DomainError::ContainerRequired.into())
    }
}

pub fn area_of(living: &Entry) -> Result<EntityPtr> {
    let occupying = living.scope::<Occupying>()?;

    Ok(occupying.area.into_entity()?)
}

pub fn set_container(container: &Entry, items: &Vec<Entry>) -> Result<()> {
    let mut containing = container.scope_mut::<Containing>()?;
    for item in items {
        containing.start_carrying(item)?;
        let mut location = item.scope_mut::<Location>()?;
        location.container = Some(container.try_into()?);
        location.save()?;
    }
    containing.save()
}

pub fn set_occupying(area: &Entry, living: &Vec<Entry>) -> Result<()> {
    let mut occupyable = area.scope_mut::<Occupyable>()?;
    for item in living {
        occupyable.start_occupying(item)?;
        let mut occupying = item.scope_mut::<Occupying>()?;
        occupying.area = area.try_into()?;
        occupying.save()?;
    }
    occupyable.save()
}

pub fn contained_by(container: &Entry) -> Result<Vec<Entry>> {
    let mut entities: Vec<Entry> = vec![];
    if let Ok(containing) = container.scope::<Containing>() {
        for entity in &containing.holding {
            entities.push(entity.into_entry()?);
        }
    }

    Ok(entities)
}

pub fn leads_to<'a>(route: &'a Entry, area: &'a Entry) -> Result<&'a Entry> {
    let mut exit = route.scope_mut::<Exit>()?;
    exit.area = area.try_into()?;
    exit.save()?;

    Ok(route)
}

pub fn get_occupant_keys(area: &Entry) -> Result<Vec<EntityKey>> {
    let occupyable = area.scope::<Occupyable>()?;

    Ok(occupyable
        .occupied
        .iter()
        .map(|e| e.key.clone())
        .collect::<Vec<EntityKey>>())
}

pub fn new_entity_from_template_ptr(template_entry: &Entry) -> Result<Entry> {
    let template = template_entry.entity()?;
    let entity = EntityPtr::new(Entity::new_from(&template.borrow())?);
    get_my_session()?.add_entity(&entity)
}

pub fn set_quantity(entity: &Entry, quantity: f32) -> Result<&Entry> {
    let mut carryable = entity.scope_mut::<Carryable>()?;
    carryable.set_quantity(quantity)?;
    carryable.save()?;

    Ok(entity)
}

pub fn separate(entity: Entry, quantity: f32) -> Result<(Entry, Entry)> {
    let kind = {
        let mut carryable = entity.scope_mut::<Carryable>()?;
        carryable.decrease_quantity(quantity)?;
        carryable.save()?;
        carryable.kind().clone()
    };

    let separated = new_entity_from_template_ptr(&entity)?;

    let mut carryable = separated.scope_mut::<Carryable>()?;

    // TODO Would be nice if we could pass this in and avoid creating one
    // unnecessarily. See comments in Entity::new_from
    carryable.set_kind(&kind);
    carryable.set_quantity(quantity)?;
    carryable.save()?;

    Ok((entity, separated))
}
