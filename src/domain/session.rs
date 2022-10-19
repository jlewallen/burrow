use crate::kernel::*;
use crate::plugins::{moving::model::Occupying, users::model::Usernames};
use crate::storage::{EntityStorage, EntityStorageFactory};
use anyhow::Result;
use std::{fmt::Debug, rc::Rc};
use tracing::{debug, event, info, span, Level};

use super::eval;
use super::internal::DomainInfrastructure;

#[derive(Debug)]
pub struct Session {
    infra: Rc<DomainInfrastructure>,
    discoverying: bool,
}

impl Session {
    pub fn new(storage: Box<dyn EntityStorage>) -> Result<Self> {
        info!("session-new");

        Ok(Self {
            infra: DomainInfrastructure::new(storage),
            discoverying: true,
        })
    }

    pub fn evaluate_and_perform(&self, user_name: &str, text: &str) -> Result<Box<dyn Reply>> {
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
    }

    pub fn hydrate_user_session(&self) -> Result<()> {
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

        Session::new(storage)
    }
}
