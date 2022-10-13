use crate::eval;
use crate::kernel::*;
use crate::plugins::users::model::Usernames;
use crate::storage::EntityStorage;
use anyhow::Result;
use std::collections::HashMap;
use tracing::{debug, info};

pub struct Session {
    storage: Box<dyn EntityStorage>,
    entities: HashMap<EntityKey, Entity>,
}

impl Session {
    pub fn evaluate_and_perform(&self, text: &str) -> Result<()> {
        debug!("session-do '{}'", text);

        let action = eval::evaluate(text)?;
        let _performed = action.perform()?;

        Ok(())
    }

    pub fn close(&self) {
        info!("session-close");
    }

    pub fn hydrate_user_session() {}
}

impl Drop for Session {
    fn drop(&mut self) {
        info!("session-drop");
    }
}

pub struct Domain {
    // storage: Box<dyn EntityStorage>,
}

use crate::storage;

impl Domain {
    pub fn new() -> Self {
        info!("domain-new");

        Domain {
            // TODO Consider making this a factory.
            // storage: Box::new(sqlite::SqliteStorage::new("world.sqlite3")),
        }
    }

    pub fn open_session(&self) -> Result<Session> {
        info!("session-open");

        // TODO Consider using factory in Domain.
        let storage = Box::new(storage::sqlite::SqliteStorage::new("world.sqlite3"));

        let world = storage.load(&WORLD_KEY)?;

        let usernames = world.scope::<Usernames>()?;

        info!("usernames: {:?}", usernames);

        // TODO get user
        // TODO get Area
        // TODO discover
        // TODO hydrate

        Ok(Session {
            storage: storage,
            entities: HashMap::new(),
        })
    }
}
