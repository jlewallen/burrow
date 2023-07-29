use anyhow::{anyhow, Result};
use std::cell::RefCell;
use std::rc::Rc;
use std::rc::Weak;
use std::sync::Arc;
use tracing::*;

use super::Session;
use crate::user_name_to_entry;
use kernel::*;

pub struct StandardPerformer {
    session: Weak<Session>,
    finder: Arc<dyn Finder>,
    plugins: Arc<RefCell<SessionPlugins>>,
    middleware: Rc<Vec<Rc<dyn Middleware>>>,
    user: Option<String>,
    target: Option<Rc<dyn Performer>>,
}

struct MakeSurroundings {
    finder: Arc<dyn Finder>,
    living: Entry,
}

impl TryInto<Surroundings> for MakeSurroundings {
    type Error = DomainError;

    fn try_into(self) -> std::result::Result<Surroundings, Self::Error> {
        let world = self.finder.find_world()?;
        let living = self.living.clone();
        let area: Entry = self.finder.find_location(&living)?;

        Ok(Surroundings::Living {
            world,
            living,
            area,
        })
    }
}

impl StandardPerformer {
    pub fn new(
        session: &Weak<Session>,
        finder: Arc<dyn Finder>,
        plugins: Arc<RefCell<SessionPlugins>>,
        middleware: Rc<Vec<Rc<dyn Middleware>>>,
        user: Option<String>,
        target: Option<Rc<dyn Performer>>,
    ) -> Rc<Self> {
        Rc::new(StandardPerformer {
            session: Weak::clone(session),
            finder,
            plugins,
            middleware,
            user,
            target,
        })
    }

    fn evaluate_living_surroundings(&self, living: &Entry) -> Result<Surroundings, DomainError> {
        let make = MakeSurroundings {
            finder: self.finder.clone(),
            living: living.clone(),
        };
        make.try_into()
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

                let living = user_name_to_entry(self.session()?.as_ref(), user)?;

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

                apply_middleware(&self.middleware, perform, request_fn)
            }
            Perform::Raised(raised) => {
                let target = self.target.clone().unwrap();
                let request_fn = Box::new(|value: Perform| -> Result<Effect, anyhow::Error> {
                    target.perform(value)
                });

                apply_middleware(&self.middleware, Perform::Raised(raised), request_fn)
            }
            Perform::Schedule(scheduling) => {
                let target = self.target.clone().unwrap();
                let request_fn = Box::new(move |value: Perform| -> Result<Effect, anyhow::Error> {
                    target.perform(value)
                });

                apply_middleware(&self.middleware, Perform::Schedule(scheduling), request_fn)
            }
            _ => todo!(),
        }
    }
}
