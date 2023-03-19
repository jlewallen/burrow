use anyhow::Result;
use std::rc::Rc;
use std::rc::Weak;
use tracing::{debug, event, info, span, Level};

use super::Session;
use crate::kernel::*;
use crate::plugins::{moving::model::Occupying, users::model::Usernames};

pub struct StandardPerformer {
    session: Weak<Session>,
}

impl StandardPerformer {
    pub fn new(session: &Weak<Session>) -> Rc<Self> {
        Rc::new(StandardPerformer {
            session: Weak::clone(session),
        })
    }

    pub fn perform_via_name(&self, name: &str, action: Box<dyn Action>) -> Result<Box<dyn Reply>> {
        info!("performing {:?}", action);

        let surroundings = self.evaluate_name(name)?;

        let reply = {
            let _span = span!(Level::INFO, "A").entered();
            action.perform(self.session()?, &surroundings)?
        };

        Ok(reply)
    }

    pub fn evaluate_and_perform(&self, name: &str, text: &str) -> Result<Option<Box<dyn Reply>>> {
        let _doing_span = span!(Level::INFO, "session-do", user = name).entered();

        debug!("'{}'", text);

        let session = self.session()?;
        if let Some(action) = session.plugins().evaluate(text)? {
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
            }) => Ok(Some(living.key().clone())),
            Err(DomainError::EntityNotFound) => Ok(None),
            Err(err) => Err(err),
        }
    }

    fn session(&self) -> Result<Rc<Session>, DomainError> {
        self.session.upgrade().ok_or(DomainError::NoSession)
    }

    fn evaluate_name(&self, name: &str) -> Result<Surroundings, DomainError> {
        let _span = span!(Level::DEBUG, "L").entered();

        let session = self.session()?;

        let world = session.world()?;

        let usernames = world.scope::<Usernames>()?;

        let user_key = &usernames.users[name];

        let living = session
            .entry(&LookupBy::Key(user_key))?
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

    pub fn perform(&self, living: &Entry, action: Box<dyn Action>) -> Result<Box<dyn Reply>> {
        info!("performing {:?}", action);

        let surroundings = self.evaluate_living(living)?;

        let reply = {
            let _span = span!(Level::INFO, "A").entered();
            action.perform(self.session()?, &surroundings)?
        };

        event!(Level::INFO, "done");

        Ok(reply)
    }
}
