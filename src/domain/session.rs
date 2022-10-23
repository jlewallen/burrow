use crate::kernel::*;
use crate::plugins::{moving::model::Occupying, users::model::Usernames};
use crate::storage::{EntityStorage, EntityStorageFactory};
use anyhow::Result;
use std::rc::Rc;
use tracing::{debug, event, info, span, Level};

use super::eval;
use super::internal::DomainInfrastructure;

pub struct Session {
    infra: Rc<DomainInfrastructure>,
    storage: Rc<dyn EntityStorage>,
    discoverying: bool,
}

impl Session {
    pub fn new(storage: Rc<dyn EntityStorage>) -> Result<Self> {
        info!("session-new");

        Ok(Self {
            infra: DomainInfrastructure::new(Rc::clone(&storage)),
            storage: storage,
            discoverying: true,
        })
    }

    pub fn evaluate_and_perform(&self, user_name: &str, text: &str) -> Result<Box<dyn Reply>> {
        let check = {
            let _doing_span = span!(Level::INFO, "session-do", user = user_name).entered();

            debug!("'{}'", text);

            let action = eval::evaluate(text)?;

            info!("performing {:?}", action);

            let (world, user, area) = {
                let _span = span!(Level::DEBUG, "L").entered();

                let world = self.infra.load_entity_by_key(&WORLD_KEY)?;
                let usernames: OpenScope<Usernames> = {
                    let world = world.borrow();
                    world.scope::<Usernames>()?
                };
                let user_key = &usernames.users[user_name];
                let user = self.infra.load_entity_by_key(user_key)?;
                let area: EntityPtr = {
                    let user = user.borrow();
                    let occupying: OpenScope<Occupying> = user.scope::<Occupying>()?;
                    occupying.area.into_entity()?
                };

                info!("area {}", area.borrow());

                (world, user, area)
            };

            if self.discoverying {
                let _span = span!(Level::DEBUG, "D").entered();
                let mut discovered_keys: Vec<EntityKey> = vec![];
                eval::discover(&user.borrow(), &mut discovered_keys)?;
                eval::discover(&area.borrow(), &mut discovered_keys)?;
                info!("discovered {:?}", discovered_keys);
            }

            let reply = {
                let _span = span!(Level::INFO, "A").entered();
                action.perform((world, user, area, self.infra.clone()))?
            };

            event!(Level::INFO, "done");

            Ok(reply)
        };

        match check {
            Ok(i) => Ok(i),
            Err(original_err) => {
                if let Err(_rollback_err) = self.storage.rollback() {
                    panic!("error rolling back");
                }
                Err(original_err)
            }
        }
    }

    pub fn close(&self) -> Result<()> {
        self.storage.commit()?;

        Ok(())
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        info!("session-drop");
    }
}

pub struct Domain {
    storage_factory: Box<dyn EntityStorageFactory>,
}

impl Domain {
    pub fn new(storage_factory: Box<dyn EntityStorageFactory>) -> Self {
        info!("domain-new");

        Domain { storage_factory }
    }

    pub fn open_session(&self) -> Result<Session> {
        info!("session-open");

        let storage = self.storage_factory.create_storage()?;

        let _ = storage.begin()?;

        Session::new(storage)
    }
}
