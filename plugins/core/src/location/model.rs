use crate::library::model::*;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Location {
    pub container: Option<EntityRef>,
}

impl Location {
    pub fn set(item: &EntityPtr, container: EntityRef) -> Result<(), DomainError> {
        let mut location = item.scope_mut::<Location>()?;
        location.container = Some(container);
        location.save()
    }

    pub fn get(item: &EntityPtr) -> Result<Option<EntityRef>, DomainError> {
        match item.scope::<Location>()? {
            Some(location) => Ok(location.container.clone()),
            None => Ok(None),
        }
    }
}

impl Scope for Location {
    fn scope_key() -> &'static str {
        "location"
    }
}

pub fn change_location<A, B, C, D>(
    from: &EntityPtr,
    to: &EntityPtr,
    item: &EntityPtr,
    do_from: C,
    do_into: D,
) -> Result<bool, DomainError>
where
    A: Scope + Serialize,
    B: Scope + Serialize,
    C: FnOnce(&mut A, EntityPtr) -> Result<Option<EntityPtr>, DomainError>,
    D: FnOnce(&mut B, EntityPtr) -> Result<Option<EntityPtr>, DomainError>,
{
    info!("moving {:?} {:?} {:?}", item, from, to);

    let mut from = from.scope_mut::<A>()?;
    let mut into = to.scope_mut::<B>()?;

    match do_from(&mut from, item.clone())? {
        Some(moving) => match do_into(&mut into, moving)? {
            Some(moving) => {
                Location::set(&moving, to.entity_ref())?;
                from.save()?;
                into.save()?;

                Ok(true)
            }
            None => Ok(false),
        },
        None => Ok(false),
    }
}

pub fn container_of(item: &EntityPtr) -> Result<EntityPtr, DomainError> {
    Location::get(item)?
        .ok_or(DomainError::ContainerRequired)?
        .to_entity()
}
