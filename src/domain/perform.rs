use anyhow::Result;
use std::rc::Rc;
use std::rc::Weak;
use tracing::{debug, event, info, span, Level};

use super::Session;
use crate::plugins::{moving::model::Occupying, users::model::Usernames};
use crate::{kernel::*, plugins::eval};

pub struct StandardPerformer {
    session: Weak<Session>,
    discoverying: bool,
}

impl StandardPerformer {
    pub fn new(session: &Weak<Session>) -> Rc<Self> {
        Rc::new(StandardPerformer {
            session: Weak::clone(session),
            discoverying: false,
        })
    }

    fn session(&self) -> Result<Rc<Session>, DomainError> {
        self.session.upgrade().ok_or(DomainError::NoSession)
    }

    pub fn perform_via_name(&self, name: &str, action: Box<dyn Action>) -> Result<Box<dyn Reply>> {
        info!("performing {:?}", action);

        let surroundings = self.evaluate_name(name)?;

        self.discover_from(surroundings.to_discovery_vec())?;

        let reply = {
            let _span = span!(Level::INFO, "A").entered();
            action.perform(ActionArgs::new(surroundings, self.session()?))?
        };

        Ok(reply)
    }

    pub fn evaluate_and_perform(&self, name: &str, text: &str) -> Result<Option<Box<dyn Reply>>> {
        let _doing_span = span!(Level::INFO, "session-do", user = name).entered();

        debug!("'{}'", text);

        if let Some(action) = eval::evaluate(self.session()?.plugins(), text)? {
            Ok(Some(self.perform_via_name(name, action)?))
        } else {
            Ok(None)
        }
    }

    pub fn find_name_key(&self, name: &str) -> Result<Option<EntityKey>, DomainError> {
        match self.evaluate_name(name) {
            Ok(Surroundings::Living {
                world: _world,
                living,
                area: _area,
            }) => Ok(Some(living.key())),
            Err(DomainError::EntityNotFound) => Ok(None),
            Err(err) => Err(err),
        }
    }

    fn evaluate_name(&self, name: &str) -> Result<Surroundings, DomainError> {
        let _span = span!(Level::DEBUG, "L").entered();

        let session = self.session()?;

        let world = session.world()?;

        let usernames = world.scope::<Usernames>()?;

        let user_key = &usernames.users[name];

        let living = session
            .entry(user_key)?
            .ok_or(DomainError::EntityNotFound)?;

        self.evaluate_living(&living)
    }

    fn evaluate_living(&self, living: &Entry) -> Result<Surroundings, DomainError> {
        let session = self.session()?;

        let world = session.world()?;

        let area: Entry = {
            let occupying = living.scope::<Occupying>()?;
            occupying.area.into_entry()?
        };

        info!("area {:?}", &area);

        Ok(Surroundings::Living {
            world,
            living: living.clone(),
            area,
        })
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

    pub fn perform(&self, living: &Entry, action: Box<dyn Action>) -> Result<Box<dyn Reply>> {
        info!("performing {:?}", action);

        let surroundings = self.evaluate_living(living)?;

        self.discover_from(surroundings.to_discovery_vec())?;

        let reply = {
            let _span = span!(Level::INFO, "A").entered();
            let session = self.session()?;
            action.perform(ActionArgs::new(surroundings, session))?
        };

        event!(Level::INFO, "done");

        Ok(reply)
    }
}
