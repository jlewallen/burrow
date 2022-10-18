use crate::kernel::*;
use crate::plugins::{
    carrying::model::Containing, moving::model::Occupying, users::model::Usernames,
};
use crate::storage::{EntityStorage, EntityStorageFactory};
use anyhow::Result;
use std::{fmt::Debug, rc::Rc};
use tracing::{debug, info, span, Level};

use super::eval;
use super::internal::DomainInfrastructure;

#[derive(Debug)]
pub struct Session {
    infra: Rc<DomainInfrastructure>,
}

impl Session {
    pub fn new(storage: Box<dyn EntityStorage>) -> Result<Self> {
        info!("session-new");

        Ok(Self {
            infra: DomainInfrastructure::new(storage),
        })
    }

    pub fn evaluate_and_perform(&self, user_name: &str, text: &str) -> Result<Box<dyn Reply>> {
        let _doing_span = span!(Level::INFO, "session-do", user = user_name).entered();

        debug!("'{}'", text);

        let action = eval::evaluate(text)?;

        info!("performing {:?}", action);

        let world = self.infra.load_entity_by_key(&WORLD_KEY)?;

        let usernames: Box<Usernames> = world.scope::<Usernames>()?;

        let user_key = &usernames.users[user_name];

        let user = self.infra.load_entity_by_key(user_key)?;

        let occupying: Box<Occupying> = user.scope::<Occupying>()?;

        let area: Box<Entity> = occupying.area.try_into()?;

        info!(%user_name, "area {}", area);

        if true {
            let _test_span = span!(Level::INFO, "test").entered();

            let containing = area.scope::<Containing>()?;
            for here in containing.holding {
                info!("here {:?}", here.key())
            }

            let carrying = user.scope::<Containing>()?;
            for here in carrying.holding {
                info!("here {:?}", here.key())
            }

            let mut discovered_keys: Vec<EntityKey> = vec![];
            eval::discover(user, &mut discovered_keys)?;
            eval::discover(area.as_ref(), &mut discovered_keys)?;
            info!(%user_name, "discovered {:?}", discovered_keys);
        }

        let reply = action.perform((world, user, &area))?;

        info!(%user_name, "done {:?}", reply);

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
