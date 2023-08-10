use crate::library::model::*;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Location {
    pub container: Option<EntityRef>,
}

impl Location {
    pub fn set(item: &Entry, container: EntityRef) -> Result<(), DomainError> {
        let mut location = item.scope_mut::<Location>()?;
        location.container = Some(container);
        location.save()
    }

    pub fn get(item: &Entry) -> Result<Option<EntityRef>, DomainError> {
        let location = item.scope::<Location>()?;
        Ok(location.container.clone())
    }
}

impl Scope for Location {
    fn serialize(&self) -> Result<JsonValue> {
        Ok(serde_json::to_value(self)?)
    }

    fn scope_key() -> &'static str {
        "location"
    }
}

impl Needs<SessionRef> for Location {
    fn supply(&mut self, session: &SessionRef) -> Result<()> {
        self.container = session.ensure_optional_entity(&self.container)?;
        Ok(())
    }
}

pub fn change_location<A, B, C, D>(
    from: &Entry,
    to: &Entry,
    item: &Entry,
    do_from: C,
    do_into: D,
) -> Result<DomainOutcome, DomainError>
where
    A: Scope,
    B: Scope,
    C: FnOnce(&mut A, Entry) -> Result<Option<Entry>>,
    D: FnOnce(&mut B, Entry) -> Result<Option<Entry>>,
{
    info!("moving {:?} {:?} {:?}", item, from, to);

    let mut from = from.scope_mut::<A>()?;
    let mut into = to.scope_mut::<B>()?;

    match do_from(&mut from, item.clone())? {
        Some(moving) => match do_into(&mut into, moving)? {
            Some(moving) => {
                Location::set(&moving, to.try_into()?)?;
                from.save()?;
                into.save()?;

                Ok(DomainOutcome::Ok)
            }
            None => Ok(DomainOutcome::Nope),
        },
        None => Ok(DomainOutcome::Nope),
    }
}

pub fn container_of(item: &Entry) -> Result<Entry, DomainError> {
    Ok(Location::get(item)?
        .ok_or(DomainError::ContainerRequired)?
        .to_entry()?)
}
