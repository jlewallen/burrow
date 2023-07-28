use anyhow::{anyhow, Context, Result};
use std::cell::RefCell;
use std::rc::Rc;
use std::rc::Weak;
use std::sync::Arc;
use std::time::Instant;
use tracing::*;

use super::Session;
use crate::users::model::HasUsernames;
use kernel::*;

pub struct StandardPerformer {
    session: Weak<Session>,
    finder: Arc<dyn Finder>,
    plugins: Arc<RefCell<SessionPlugins>>,
    middleware: Rc<Vec<Rc<dyn Middleware>>>,
    user: Option<String>,
}

impl StandardPerformer {
    pub fn new(
        session: &Weak<Session>,
        finder: Arc<dyn Finder>,
        plugins: Arc<RefCell<SessionPlugins>>,
        middleware: Rc<Vec<Rc<dyn Middleware>>>,
        user: Option<String>,
    ) -> Rc<Self> {
        Rc::new(StandardPerformer {
            session: Weak::clone(session),
            finder,
            plugins,
            middleware,
            user,
        })
    }

    pub fn evaluate_and_perform(&self, name: &str, text: &str) -> Result<Option<Effect>> {
        let started = Instant::now();
        let _doing_span = span!(Level::DEBUG, "do", user = name).entered();

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
            middleware: Rc::clone(&self.middleware),
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
        let _span = span!(Level::DEBUG, "P").entered();

        debug!("perform {:?}", perform);

        match perform {
            Perform::Chain(action) => {
                let Some(user) = &self.user else {
                    return Err(anyhow!("No active user in StandardPerformer"));
                };

                info!("perform:chain {:?}", action);

                let living = self.evaluate_living(user)?;

                self.perform(Perform::Living { living, action })
            }
            Perform::Living { living, action } => {
                info!("perform:living");

                let surroundings = {
                    let surroundings = self.evaluate_living_surroundings(&living)?;
                    info!("surroundings {:?}", &surroundings);
                    let plugins = self.plugins.borrow();
                    plugins.have_surroundings(&surroundings)?;
                    surroundings
                };

                let request_fn = Box::new(|value: Perform| -> Result<Effect, anyhow::Error> {
                    let _span = span!(Level::DEBUG, "A").entered();
                    if let Perform::Chain(action) = value {
                        info!("action:perform {:?}", &action);
                        let res = action.perform(self.session()?, &surroundings);
                        if let Ok(effect) = &res {
                            trace!("action:effect {:?}", effect);
                            info!("action:effect");
                        } else {
                            warn!("action:error {:?}", res);
                        }
                        res
                    } else {
                        todo!()
                    }
                });

                let perform = Perform::Chain(action);

                self.session()?.take_snapshot()?;

                apply_middleware(&self.middleware, perform, request_fn)
            }
            _ => todo!(),
        }
    }
}
