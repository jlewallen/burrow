use crate::eval;
use crate::kernel::*;
use crate::plugins::users::model::Usernames;
use crate::storage::EntityStorage;
use anyhow::Result;
use elsa::FrozenMap;
use tracing::{debug, info};

pub struct Session {
    storage: Box<dyn EntityStorage>,
    entities: FrozenMap<EntityKey, Box<Entity>>,
}

impl Session {
    pub fn new(storage: Box<dyn EntityStorage>) -> Self {
        info!("session-new");

        Self {
            storage: storage,
            entities: FrozenMap::new(),
        }
    }

    pub fn load_entity_by_key(&self, key: &EntityKey) -> Result<&Entity> {
        if let Some(e) = self.entities.get(key) {
            debug!(%key, "loading local entity");
            return Ok(e);
        }

        debug!(%key, "loading entity");

        let loaded = self.storage.load(key)?;

        let inserted = self.entities.insert(key.clone(), Box::new(loaded));

        Ok(inserted)
    }

    pub fn evaluate_and_perform(&self, user_name: &str, text: &str) -> Result<()> {
        debug!(%user_name, "session-do '{}'", text);

        let world = self.load_entity_by_key(&WORLD_KEY)?;

        let usernames = world.scope::<Usernames>()?;

        let user_key = &usernames.users[user_name];

        let user = self.load_entity_by_key(user_key)?;

        let action = eval::evaluate(text)?;

        let performed = action.perform((&world, &user))?;

        info!(%user_name, "done {:?}", performed);

        Ok(())
    }

    pub fn hydrate_user_session(&self) -> Result<()> {
        Ok(())
    }

    pub fn close(&self) {
        info!("session-close");
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        info!("session-drop");
    }
}

pub struct Domain {
    // storage_factory: Box<dyn EntityStorageFactory>,
}

use crate::storage;

impl Domain {
    pub fn new() -> Self {
        info!("domain-new");

        Domain {}
    }

    pub fn open_session(&self) -> Result<Session> {
        info!("session-open");

        // TODO Consider using factory in Domain.
        let storage = Box::new(storage::sqlite::SqliteStorage::new("world.sqlite3"));

        let session = Session::new(storage);

        // TODO get user
        // TODO get Area
        // TODO discover
        // TODO hydrate

        Ok(session)
    }
}
