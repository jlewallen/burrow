use anyhow::Result;
use tracing::info;

use super::{
    carrying::model::{Carryable, Containing},
    fashion::model::Wearing,
    location::{change_location, Location},
    moving::model::{Exit, Occupyable, Occupying},
};
use kernel::prelude::*;

pub use super::location::container_of;

pub fn is_container(item: &EntityPtr) -> Result<bool, DomainError> {
    Ok(item.scope::<Containing>()?.is_some())
}

pub fn wear_article(
    from: &EntityPtr,
    to: &EntityPtr,
    item: &EntityPtr,
) -> Result<DomainOutcome, DomainError> {
    change_location(
        from,
        to,
        item,
        |s: &mut Containing, item: EntityPtr| s.stop_carrying(&item),
        |s: &mut Wearing, item: EntityPtr| {
            s.start_wearing(&item)?;
            Ok(Some(item))
        },
    )
}

pub fn remove_article(
    from: &EntityPtr,
    to: &EntityPtr,
    item: &EntityPtr,
) -> Result<DomainOutcome, DomainError> {
    change_location(
        from,
        to,
        item,
        |s: &mut Wearing, item: EntityPtr| s.stop_wearing(&item),
        |s: &mut Containing, item: EntityPtr| {
            s.start_carrying(&item)?;
            Ok(Some(item))
        },
    )
}

pub fn move_between(
    from: &EntityPtr,
    to: &EntityPtr,
    item: &EntityPtr,
) -> Result<DomainOutcome, DomainError> {
    change_location(
        from,
        to,
        item,
        |s: &mut Containing, item: EntityPtr| s.stop_carrying(&item),
        |s: &mut Containing, item: EntityPtr| {
            s.start_carrying(&item)?;
            Ok(Some(item.clone()))
        },
    )
}

pub fn navigate_between(
    from: &EntityPtr,
    to: &EntityPtr,
    item: &EntityPtr,
) -> Result<DomainOutcome, DomainError> {
    info!("navigating {:?}", item);

    let mut from = from.scope_mut::<Occupyable>()?;
    let mut into = to.scope_mut::<Occupyable>()?;

    match from.stop_occupying(item)? {
        DomainOutcome::Ok => {
            let mut location = item.scope_mut::<Occupying>()?;
            location.area = to.entity_ref();

            into.start_occupying(item)?;
            into.save()?;
            from.save()?;
            location.save()?;

            Ok(DomainOutcome::Ok)
        }
        DomainOutcome::Nope => Ok(DomainOutcome::Nope),
    }
}

pub fn area_of(living: &EntityPtr) -> Result<EntityPtr, DomainError> {
    let occupying = living.scope::<Occupying>()?.unwrap();

    occupying.area.to_entity()
}

pub fn get_contained_keys(area: &EntityPtr) -> Result<Vec<EntityKey>, DomainError> {
    let containing = area.scope::<Containing>()?.unwrap();

    Ok(containing
        .holding
        .iter()
        .map(|e| e.key().clone())
        .collect::<Vec<EntityKey>>())
}

pub fn set_wearing(container: &EntityPtr, items: &Vec<EntityPtr>) -> Result<(), DomainError> {
    let mut wearing = container.scope_mut::<Wearing>()?;
    for item in items {
        wearing.start_wearing(item)?;
        Location::set(item, container.entity_ref())?;
    }
    wearing.save()
}

pub fn set_container(container: &EntityPtr, items: &Vec<EntityPtr>) -> Result<(), DomainError> {
    let mut containing = container.scope_mut::<Containing>()?;
    for item in items {
        containing.start_carrying(item)?;
        Location::set(item, container.entity_ref())?;
    }
    containing.save()
}

pub fn set_occupying(area: &EntityPtr, living: &Vec<EntityPtr>) -> Result<(), DomainError> {
    let mut occupyable = area.scope_mut::<Occupyable>()?;
    for item in living {
        occupyable.start_occupying(item)?;
        let mut occupying = item.scope_mut::<Occupying>()?;
        occupying.area = area.entity_ref();
        occupying.save()?;
    }
    occupyable.save()
}

pub fn contained_by(container: &EntityPtr) -> Result<Vec<EntityPtr>, DomainError> {
    let mut entities: Vec<EntityPtr> = vec![];
    if let Ok(Some(containing)) = container.scope::<Containing>() {
        for entity in &containing.holding {
            entities.push(entity.to_entity()?);
        }
    }

    Ok(entities)
}

pub fn leads_to<'a>(route: &'a EntityPtr, area: &'a EntityPtr) -> Result<&'a EntityPtr> {
    let mut exit = route.scope_mut::<Exit>()?;
    exit.area = area.entity_ref();
    exit.save()?;

    Ok(route)
}

pub fn occupied_by(area: &EntityPtr) -> Result<Vec<EntityPtr>> {
    let occupyable = area.scope::<Occupyable>()?.unwrap();

    occupyable
        .occupied
        .iter()
        .map(|e| Ok(e.to_entity()?))
        .collect::<Result<Vec<_>>>()
}

pub fn get_occupant_keys(area: &EntityPtr) -> Result<Vec<EntityKey>> {
    let occupyable = area.scope::<Occupyable>()?.unwrap();

    Ok(occupyable
        .occupied
        .iter()
        .map(|e| e.key().clone())
        .collect::<Vec<EntityKey>>())
}

pub fn new_entity_from_template_ptr(template_entry: &EntityPtr) -> Result<EntityPtr> {
    let template = template_entry.entity();
    let key = get_my_session()?.new_key();
    let entity = build_entity()
        .with_key(key)
        .copying(&template.borrow())?
        .try_into()?;
    get_my_session()?.add_entity(entity)
}

pub fn quantity(entity: &EntityPtr) -> Result<f32> {
    let carryable = entity.scope::<Carryable>()?.unwrap();
    Ok(carryable.quantity())
}

pub fn set_quantity(entity: &EntityPtr, quantity: f32) -> Result<&EntityPtr> {
    let mut carryable = entity.scope_mut::<Carryable>()?;
    carryable.set_quantity(quantity)?;
    carryable.save()?;

    Ok(entity)
}

pub fn separate(entity: &EntityPtr, quantity: f32) -> Result<(&EntityPtr, EntityPtr)> {
    let kind = {
        let mut carryable = entity.scope_mut::<Carryable>()?;
        carryable.decrease_quantity(quantity)?;
        carryable.save()?;
        carryable.kind().clone()
    };

    let separated = new_entity_from_template_ptr(entity)?;

    {
        let mut carryable = separated.scope_mut::<Carryable>()?;

        // TODO Would be nice if we could pass 'Kind' in to the ctor and avoid
        // creating one unnecessarily. See comments in Entity::new_from
        carryable.set_kind(&kind);
        carryable.set_quantity(quantity)?;
        carryable.save()?;
    }

    Ok((entity, separated))
}

pub fn duplicate(entity: &EntityPtr) -> Result<EntityPtr> {
    let mut carryable = entity.scope_mut::<Carryable>()?;
    carryable.increase_quantity(1.0)?;
    carryable.save()?;

    Ok(entity.clone())
}

pub fn obliterate(obliterating: &EntityPtr) -> Result<()> {
    // NOTE: It's very easy to get confused about which entity is which.
    let location = obliterating.scope::<Location>()?.unwrap();
    if let Some(container) = &location.container {
        let container = container.to_entity()?;
        let mut containing = container.scope_mut::<Containing>()?;

        containing.stop_carrying(obliterating)?;
        containing.save()?;

        get_my_session()?.obliterate(obliterating)?;

        Ok(())
    } else {
        Err(DomainError::ContainerRequired.into())
    }
}

pub fn get_adjacent_keys(entry: &EntityPtr) -> Result<Vec<EntityKey>> {
    let containing = entry.scope::<Containing>()?.unwrap();

    Ok(containing
        .holding
        .iter()
        .map(|e| e.to_entity())
        .collect::<Result<Vec<EntityPtr>, kernel::prelude::DomainError>>()?
        .into_iter()
        .map(|e| {
            if let Some(exit) = e.scope::<Exit>()? {
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

pub fn worn_by(wearer: &EntityPtr) -> Result<Option<Vec<EntityPtr>>, DomainError> {
    let Ok(Some(wearing)) = wearer.scope::<Wearing>() else {
        return Ok(None);
    };

    let mut entities: Vec<EntityPtr> = vec![];
    for entity in &wearing.wearing {
        entities.push(entity.to_entity()?);
    }
    Ok(Some(entities))
}

pub fn holding_one_item(container: &EntityPtr) -> Result<Option<EntityPtr>, DomainError> {
    let holding = contained_by(container)?;
    if holding.len() == 1 {
        Ok(holding.into_iter().next())
    } else {
        Ok(None)
    }
}
