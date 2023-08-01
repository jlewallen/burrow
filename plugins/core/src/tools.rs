use anyhow::Result;
use tracing::info;

use super::{
    carrying::model::{Carryable, Containing},
    fashion::model::Wearing,
    location::{change_location, Location},
    moving::model::{Exit, Occupyable, Occupying},
};
use kernel::{get_my_session, model::*, DomainOutcome, EntityPtr};

pub use super::location::container_of;

pub fn is_container(item: &Entry) -> Result<bool, DomainError> {
    item.has_scope::<Containing>()
}

pub fn wear_article(from: &Entry, to: &Entry, item: &Entry) -> Result<DomainOutcome, DomainError> {
    change_location(
        from,
        to,
        item,
        |s: &mut Containing, item: Entry| s.stop_carrying(&item),
        |s: &mut Wearing, item: Entry| {
            s.start_wearing(&item)?;
            Ok(Some(item))
        },
    )
}

pub fn remove_article(
    from: &Entry,
    to: &Entry,
    item: &Entry,
) -> Result<DomainOutcome, DomainError> {
    change_location(
        from,
        to,
        item,
        |s: &mut Wearing, item: Entry| s.stop_wearing(&item),
        |s: &mut Containing, item: Entry| {
            s.start_carrying(&item)?;
            Ok(Some(item))
        },
    )
}

pub fn move_between(from: &Entry, to: &Entry, item: &Entry) -> Result<DomainOutcome, DomainError> {
    change_location(
        from,
        to,
        item,
        |s: &mut Containing, item: Entry| s.stop_carrying(&item),
        |s: &mut Containing, item: Entry| {
            s.start_carrying(&item)?;
            Ok(Some(item.clone()))
        },
    )
}

pub fn navigate_between(
    from: &Entry,
    to: &Entry,
    item: &Entry,
) -> Result<DomainOutcome, DomainError> {
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

pub fn area_of(living: &Entry) -> Result<EntityPtr, DomainError> {
    let occupying = living.scope::<Occupying>()?;

    occupying.area.to_entity()
}

pub fn get_contained_keys(area: &Entry) -> Result<Vec<EntityKey>, DomainError> {
    let containing = area.scope::<Containing>()?;

    Ok(containing
        .holding
        .iter()
        .map(|e| e.key().clone())
        .collect::<Vec<EntityKey>>())
}

pub fn set_container(container: &Entry, items: &Vec<Entry>) -> Result<(), DomainError> {
    let mut containing = container.scope_mut::<Containing>()?;
    for item in items {
        containing.start_carrying(item)?;
        Location::set(item, container.try_into()?)?;
    }
    containing.save()
}

pub fn set_occupying(area: &Entry, living: &Vec<Entry>) -> Result<(), DomainError> {
    let mut occupyable = area.scope_mut::<Occupyable>()?;
    for item in living {
        occupyable.start_occupying(item)?;
        let mut occupying = item.scope_mut::<Occupying>()?;
        occupying.area = area.try_into()?;
        occupying.save()?;
    }
    occupyable.save()
}

pub fn contained_by(container: &Entry) -> Result<Vec<Entry>, DomainError> {
    let mut entities: Vec<Entry> = vec![];
    if let Ok(containing) = container.scope::<Containing>() {
        for entity in &containing.holding {
            entities.push(entity.to_entry()?);
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
        .map(|e| e.key().clone())
        .collect::<Vec<EntityKey>>())
}

pub fn new_entity_from_template_ptr(template_entry: &Entry) -> Result<Entry> {
    let template = template_entry.entity();
    let key = get_my_session()?.new_key();
    let entity = build_entity()
        .with_key(key)
        .copying(&template.borrow())?
        .try_into()?;
    get_my_session()?.add_entity(&EntityPtr::new(entity))
}

pub fn quantity(entity: &Entry) -> Result<f32> {
    let carryable = entity.scope::<Carryable>()?;
    Ok(carryable.quantity())
}

pub fn set_quantity(entity: &Entry, quantity: f32) -> Result<&Entry> {
    let mut carryable = entity.scope_mut::<Carryable>()?;
    carryable.set_quantity(quantity)?;
    carryable.save()?;

    Ok(entity)
}

pub fn separate(entity: &Entry, quantity: f32) -> Result<(&Entry, Entry)> {
    let kind = {
        let mut carryable = entity.scope_mut::<Carryable>()?;
        carryable.decrease_quantity(quantity)?;
        carryable.save()?;
        carryable.kind().clone()
    };

    let separated = new_entity_from_template_ptr(entity)?;

    let mut carryable = separated.scope_mut::<Carryable>()?;

    // TODO Would be nice if we could pass 'Kind' in to the ctor and avoid
    // creating one unnecessarily. See comments in Entity::new_from
    carryable.set_kind(&kind);
    carryable.set_quantity(quantity)?;
    carryable.save()?;

    Ok((entity, separated))
}

pub fn duplicate(entity: &Entry) -> Result<Entry> {
    let mut carryable = entity.scope_mut::<Carryable>()?;
    carryable.increase_quantity(1.0)?;
    carryable.save()?;

    Ok(entity.clone())
}

pub fn obliterate(obliterating: &Entry) -> Result<()> {
    // NOTE: It's very easy to get confused about which entity is which.
    let location = obliterating.scope::<Location>()?;
    if let Some(container) = &location.container {
        let container = container.to_entry()?;
        let mut containing = container.scope_mut::<Containing>()?;

        containing.stop_carrying(obliterating)?;
        containing.save()?;

        get_my_session()?.obliterate(obliterating)?;

        Ok(())
    } else {
        Err(DomainError::ContainerRequired.into())
    }
}

pub fn get_adjacent_keys(entry: &Entry) -> Result<Vec<EntityKey>> {
    let containing = entry.scope::<Containing>()?;

    Ok(containing
        .holding
        .iter()
        .map(|e| e.to_entry())
        .collect::<Result<Vec<Entry>, kernel::DomainError>>()?
        .into_iter()
        .map(|e| {
            if let Some(exit) = e.maybe_scope::<Exit>()? {
                Ok(vec![exit.area.key().clone()])
            } else {
                Ok(vec![])
            }
        })
        .collect::<Result<Vec<Vec<EntityKey>>>>()?
        .into_iter()
        .flat_map(|v| v.into_iter())
        .collect::<Vec<_>>())
}
