use anyhow::Result;
use std::rc::Rc;
use std::rc::Weak;
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, event, info, span, Level};

use super::Session;
use crate::username_to_key;
use crate::Finder;
use kernel::*;

pub struct StandardPerformer {
    session: Weak<Session>,
    finder: Arc<dyn Finder>,
    plugins: Arc<SessionPlugins>,
}

impl StandardPerformer {
    pub fn new(
        session: &Weak<Session>,
        finder: Arc<dyn Finder>,
        plugins: Arc<SessionPlugins>,
    ) -> Rc<Self> {
        Rc::new(StandardPerformer {
            session: Weak::clone(session),
            finder,
            plugins,
        })
    }

    pub fn evaluate_and_perform(&self, name: &str, text: &str) -> Result<Option<Box<dyn Reply>>> {
        let started = Instant::now();
        let _doing_span = span!(Level::INFO, "session-do", user = name).entered();

        debug!("'{}'", text);

        let res = {
            let session = self.session()?;
            if let Some(action) = session.plugins().evaluate(text)? {
                Ok(Some(self.perform_via_name(name, action)?))
            } else {
                Ok(None)
            }
        };

        let elapsed = started.elapsed();
        let elapsed = format!("{:?}", elapsed);

        info!(%elapsed, "done");

        res
    }

    fn perform_via_name(&self, name: &str, action: Box<dyn Action>) -> Result<Box<dyn Reply>> {
        info!("performing {:?}", action);

        let surroundings = self.evaluate_name(name)?;

        let reply = {
            let _span = span!(Level::INFO, "action").entered();
            info!("{:?}", &surroundings);
            self.plugins.have_surroundings(&surroundings)?;
            action.perform(self.session()?, &surroundings)?
        };

        Ok(reply)
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
        let _span = span!(Level::DEBUG, "who").entered();

        let session = self.session()?;
        let world = session.world()?;
        let user_key = username_to_key(&world, name)?.ok_or_else(|| DomainError::EntityNotFound)?;

        let living = session
            .entry(&LookupBy::Key(&user_key))?
            .ok_or(DomainError::EntityNotFound)?;

        self.evaluate_living(&living)
    }

    fn evaluate_living(&self, living: &Entry) -> Result<Surroundings, DomainError> {
        let session = self.session()?;
        let world = session.world()?;
        let area: Entry = self.finder.find_location(living)?;

        Ok(Surroundings::Living {
            world,
            living: living.clone(),
            area,
        })
    }

    pub fn perform(&self, perform: Perform) -> Result<Box<dyn Reply>> {
        info!("performing {:?}", perform);

        match &perform {
            Perform::Living { living, action } => {
                let surroundings = self.evaluate_living(living)?;

                let reply = {
                    let _span = span!(Level::INFO, "A").entered();
                    info!("{:?}", &surroundings);
                    self.plugins.have_surroundings(&surroundings)?;
                    action.perform(self.session()?, &surroundings)?
                };

                event!(Level::INFO, "done");

                Ok(reply)
            }
        }
    }
}
