use anyhow::Result;
use std::{cell::RefCell, rc::Rc};
use tracing::{debug, event, info, span, Level};

use super::internal::Performer;
use super::Entry;
use crate::plugins::{moving::model::Occupying, users::model::Usernames};
use crate::{kernel::*, plugins::eval};

pub struct StandardPerformer {
    infra: RefCell<Option<InfrastructureRef>>,
    discoverying: bool,
}

impl StandardPerformer {
    pub fn initialize(&self, infra: InfrastructureRef) {
        *self.infra.borrow_mut() = Some(infra);
    }

    pub fn new(infra: Option<InfrastructureRef>) -> Rc<Self> {
        Rc::new(StandardPerformer {
            infra: RefCell::new(infra),
            discoverying: false,
        })
    }

    pub fn perform_via_name(&self, name: &str, action: Box<dyn Action>) -> Result<Box<dyn Reply>> {
        info!("performing {:?}", action);

        let (world, user, area) = self.evaluate_name(name)?;

        self.discover_from(vec![&user, &area])?;

        let reply = {
            let _span = span!(Level::INFO, "A").entered();
            let infra = self
                .infra
                .borrow()
                .as_ref()
                .ok_or(DomainError::NoInfrastructure)?
                .clone();
            action.perform((world, user, area, infra))?
        };

        Ok(reply)
    }

    pub fn evaluate_and_perform(&self, name: &str, text: &str) -> Result<Option<Box<dyn Reply>>> {
        let _doing_span = span!(Level::INFO, "session-do", user = name).entered();

        debug!("'{}'", text);

        if let Some(action) = eval::evaluate(text)? {
            Ok(Some(self.perform_via_name(name, action)?))
        } else {
            Ok(None)
        }
    }

    pub fn find_name_key(&self, name: &str) -> Result<Option<EntityKey>, DomainError> {
        match self.evaluate_name(name) {
            Ok((_world, user, _area)) => Ok(Some(user.key())),
            Err(DomainError::EntityNotFound) => Ok(None),
            Err(err) => Err(err),
        }
    }

    fn evaluate_name(&self, name: &str) -> Result<(Entry, Entry, Entry), DomainError> {
        let _span = span!(Level::DEBUG, "L").entered();

        let infra = self.infra.borrow();

        let world = infra
            .as_ref()
            .ok_or(DomainError::NoInfrastructure)?
            .entry(&WORLD_KEY)?
            .ok_or(DomainError::EntityNotFound)?;

        let usernames = world.scope::<Usernames>()?;

        let user_key = &usernames.users[name];

        let living = infra
            .as_ref()
            .ok_or(DomainError::NoInfrastructure)?
            .load_entity_by_key(user_key)?
            .ok_or(DomainError::EntityNotFound)?;

        self.evaluate_living(&living.try_into()?)
    }

    fn evaluate_living(&self, living: &Entry) -> Result<(Entry, Entry, Entry), DomainError> {
        let world = self
            .infra
            .borrow()
            .as_ref()
            .ok_or(DomainError::NoInfrastructure)?
            .load_entity_by_key(&WORLD_KEY)?
            .ok_or(DomainError::EntityNotFound)?;

        let area: Entry = {
            let occupying = living.scope::<Occupying>()?;
            occupying.area.into_entry()?
        };

        info!("area {:?}", &area);

        Ok((world.try_into()?, living.clone(), area))
    }

    fn discover_from(&self, entities: Vec<&Entry>) -> Result<Vec<EntityKey>> {
        let _span = span!(Level::DEBUG, "D").entered();
        let mut discovered: Vec<EntityKey> = vec![];
        if self.discoverying {
            for entity in &entities {
                eval::discover(entity, &mut discovered)?;
            }
            info!("discovered {:?}", discovered);
        }
        Ok(discovered)
    }
}

impl Performer for StandardPerformer {
    fn perform(&self, living: &Entry, action: Box<dyn Action>) -> Result<Box<dyn Reply>> {
        info!("performing {:?}", action);

        let (world, living, area) = self.evaluate_living(living)?;

        self.discover_from(vec![&living, &area])?;

        let reply = {
            let _span = span!(Level::INFO, "A").entered();
            let infra = self
                .infra
                .borrow()
                .as_ref()
                .ok_or(DomainError::NoInfrastructure)?
                .clone();
            action.perform((world, living, area, infra))?
        };

        event!(Level::INFO, "done");

        Ok(reply)
    }
}
