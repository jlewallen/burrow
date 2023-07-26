use anyhow::{anyhow, Context, Result};
use std::cell::RefCell;
use std::rc::Rc;
use std::rc::Weak;
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, info, span, Level};

use super::Session;
use crate::users::model::HasUsernames;
use kernel::*;

pub struct StandardPerformer {
    session: Weak<Session>,
    finder: Arc<dyn Finder>,
    plugins: Arc<RefCell<SessionPlugins>>,
    user: Option<String>,
}

impl StandardPerformer {
    pub fn new(
        session: &Weak<Session>,
        finder: Arc<dyn Finder>,
        plugins: Arc<RefCell<SessionPlugins>>,
        user: Option<String>,
    ) -> Rc<Self> {
        Rc::new(StandardPerformer {
            session: Weak::clone(session),
            finder,
            plugins,
            user,
        })
    }

    pub fn evaluate_and_perform(&self, name: &str, text: &str) -> Result<Option<Effect>> {
        let started = Instant::now();
        let _doing_span = span!(Level::INFO, "session-do", user = name).entered();

        debug!("'{}'", text);

        let res = {
            let plugins = self.plugins.borrow();

            let as_user = self.as_user(name)?;

            Ok(plugins
                .evaluate(&as_user, Evaluable::Phrase(text))?
                .into_iter()
                .next())
        };

        let elapsed = started.elapsed();
        let elapsed = format!("{:?}", elapsed);

        info!(%elapsed, "done");

        res
    }

    fn as_user(&self, name: &str) -> Result<StandardPerformer> {
        Ok(Self {
            session: Rc::downgrade(&self.session()?),
            finder: Arc::clone(&self.finder),
            plugins: Arc::clone(&self.plugins),
            user: Some(name.to_owned()),
        })
    }

    fn evaluate_living(&self, name: &str) -> Result<Entry, DomainError> {
        let _span = span!(Level::DEBUG, "who").entered();

        let session = self.session()?;
        let world = session.world()?;
        let user_key = world
            .find_name_key(name)?
            .ok_or(DomainError::EntityNotFound)?;

        session
            .entry(&LookupBy::Key(&user_key))?
            .ok_or(DomainError::EntityNotFound)
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

    fn session(&self) -> Result<Rc<Session>, DomainError> {
        self.session.upgrade().ok_or(DomainError::NoSession)
    }
}

impl Performer for StandardPerformer {
    fn perform(&self, perform: Perform) -> Result<Effect> {
        info!("performing {:?}", perform);

        match perform {
            Perform::Living { living, action } => {
                let surroundings = self.evaluate_living_surroundings(&living)?;

                {
                    let _span = span!(Level::INFO, "S").entered();
                    info!("surroundings {:?}", &surroundings);
                    let plugins = self.plugins.borrow();
                    plugins.have_surroundings(&surroundings)?;
                }

                let reply = {
                    let _span = span!(Level::INFO, "A").entered();
                    action.perform(self.session()?, &surroundings)?
                };

                Ok(reply)
            }
            Perform::Action(action) => {
                let Some(user) = &self.user else {
                    return Err(anyhow!("No active user in StandardPerformer"));
                };

                info!("action {:?}", action);

                let living = self.evaluate_living(user)?;

                self.perform(Perform::Living { living, action })
            }
        }
    }
}
