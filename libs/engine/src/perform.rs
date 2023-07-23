use anyhow::Context;
use anyhow::Result;
use std::cell::RefCell;
use std::rc::Rc;
use std::rc::Weak;
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, info, span, Level};

use super::Session;
use crate::username_to_key;
use kernel::*;

pub struct StandardPerformer {
    session: Weak<Session>,
    finder: Arc<dyn Finder>,
    plugins: Arc<RefCell<SessionPlugins>>,
}

impl StandardPerformer {
    pub fn new(
        session: &Weak<Session>,
        finder: Arc<dyn Finder>,
        plugins: Arc<RefCell<SessionPlugins>>,
    ) -> Rc<Self> {
        Rc::new(StandardPerformer {
            session: Weak::clone(session),
            finder,
            plugins,
        })
    }

    pub fn evaluate_and_perform(&self, name: &str, text: &str) -> Result<Option<Effect>> {
        let started = Instant::now();
        let _doing_span = span!(Level::INFO, "session-do", user = name).entered();

        debug!("'{}'", text);

        let res = {
            let plugins = self.plugins.borrow();
            if let Some(action) = plugins.evaluate(text)? {
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

    fn perform_via_name(&self, name: &str, action: Box<dyn Action>) -> Result<Effect> {
        info!("action {:?}", action);

        let living = self.evaluate_living(name)?;

        self.perform(Perform::Living { living, action })
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
        let living = self.evaluate_living(name)?;
        self.evaluate_living_surroundings(&living)
    }

    fn evaluate_living(&self, name: &str) -> Result<Entry> {
        let _span = span!(Level::DEBUG, "who").entered();

        let session = self.session()?;
        let world = session.world()?;
        let user_key = username_to_key(&world, name)
            .with_context(|| "World username to key".to_string())?
            .ok_or_else(|| DomainError::EntityNotFound)
            .with_context(|| format!("Name: {}", name))?;

        session
            .entry(&LookupBy::Key(&user_key))
            .with_context(|| format!("Entry for key: {:?}", user_key))?
            .ok_or(DomainError::EntityNotFound)
            .with_context(|| format!("Key: {:?}", user_key))
    }

    fn evaluate_living_surroundings(&self, living: &Entry) -> Result<Surroundings, DomainError> {
        let session = self.session()?;
        let world = session.world()?;
        let area: Entry = self
            .finder
            .find_location(living)
            .with_context(|| format!("Location of {:?}", living))?;

        Ok(Surroundings::Living {
            world,
            living: living.clone(),
            area,
        })
    }

    pub fn perform(&self, perform: Perform) -> Result<Effect> {
        info!("performing {:?}", perform);

        match &perform {
            Perform::Living { living, action } => {
                let surroundings = self.evaluate_living_surroundings(living)?;

                {
                    let _span = span!(Level::INFO, "S").entered();
                    info!("surroundings {:?}", &surroundings);
                    let plugins = self.plugins.borrow();
                    plugins
                        .have_surroundings(&surroundings)
                        .with_context(|| format!("Evaluating: {:?}", perform))?;
                }

                let reply = {
                    let _span = span!(Level::INFO, "A").entered();
                    action.perform(self.session()?, &surroundings)?
                };

                Ok(reply)
            }
        }
    }
}
