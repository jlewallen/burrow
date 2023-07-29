use anyhow::Result;
use std::cell::RefCell;
use std::rc::Rc;
use std::rc::Weak;
use std::sync::Arc;
use tracing::*;

use super::Session;
use kernel::*;

pub struct StandardPerformer {
    session: Weak<Session>,
    finder: Arc<dyn Finder>,
    plugins: Arc<RefCell<SessionPlugins>>,
    middleware: Rc<Vec<Rc<dyn Middleware>>>,
    target: Option<Rc<dyn Performer>>,
}

impl StandardPerformer {
    pub fn new(
        session: &Weak<Session>,
        finder: Arc<dyn Finder>,
        plugins: Arc<RefCell<SessionPlugins>>,
        middleware: Rc<Vec<Rc<dyn Middleware>>>,
        target: Option<Rc<dyn Performer>>,
    ) -> Rc<Self> {
        Rc::new(StandardPerformer {
            session: Weak::clone(session),
            finder,
            plugins,
            middleware,
            target,
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
            Perform::Living { living, action } => {
                info!("perform:living");

                let surroundings = {
                    let make = MakeSurroundings {
                        finder: self.finder.clone(),
                        living: living.clone(),
                    };
                    let surroundings = make.try_into()?;
                    info!("surroundings {:?}", &surroundings);
                    let plugins = self.plugins.borrow();
                    plugins.have_surroundings(&surroundings)?;
                    surroundings
                };

                let request_fn = Box::new(|value: Perform| -> Result<Effect, anyhow::Error> {
                    let _span = span!(Level::DEBUG, "A").entered();
                    if let Perform::Surroundings {
                        surroundings,
                        action,
                    } = value
                    {
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

                apply_middleware(
                    &self.middleware,
                    Perform::Surroundings {
                        surroundings,
                        action,
                    },
                    request_fn,
                )
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
            _ => todo!("{:?}", perform),
        }
    }
}

pub struct MakeSurroundings {
    pub finder: Arc<dyn Finder>,
    pub living: Entry,
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
