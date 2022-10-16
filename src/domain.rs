use crate::eval;
use crate::kernel::*;
use crate::plugins::carrying::model::Containing;
use crate::plugins::moving::model::Occupying;
use crate::plugins::users::model::Usernames;
use crate::storage::{EntityStorage, EntityStorageFactory};
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

    pub fn load_entities_by_refs(&self, entity_refs: Vec<EntityRef>) -> Result<Vec<&Entity>> {
        entity_refs
            .into_iter()
            .map(|re| -> Result<&Entity> { self.load_entity_by_ref(&re) })
            .collect()
    }

    pub fn load_entity_by_ref(&self, entity_ref: &EntityRef) -> Result<&Entity> {
        self.load_entity_by_key(&entity_ref.key)
    }

    pub fn load_entities_by_keys(&self, entity_keys: Vec<EntityKey>) -> Result<Vec<&Entity>> {
        entity_keys
            .into_iter()
            .map(|key| -> Result<&Entity> { self.load_entity_by_key(&key) })
            .collect()
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

    pub fn evaluate_and_perform(&self, user_name: &str, text: &str) -> Result<Box<dyn Reply>> {
        debug!(%user_name, "session-do '{}'", text);

        let world = self.load_entity_by_key(&WORLD_KEY)?;

        let usernames: Box<Usernames> = world.try_into()?;

        let user_key = &usernames.users[user_name];

        let user = self.load_entity_by_key(user_key)?;

        let occupying: Box<Occupying<EntityRef>> = user.try_into()?;

        let action = eval::evaluate(text)?;

        info!(%user_name, "performing {:?}", action);

        // info!(%user_name, "user {:?}", user);

        let area = self.load_entity_by_ref(&occupying.area)?;

        info!(%user_name, "area {}", area);

        let containing = area.scope::<Containing>()?;
        for here in self.load_entities_by_refs(containing.holding)? {
            info!("here {}", here)
        }

        let carrying = user.scope::<Containing>()?;
        for here in self.load_entities_by_refs(carrying.holding)? {
            info!("here {}", here)
        }

        let mut discovered_keys: Vec<EntityKey> = vec![];
        eval::discover(user, &mut discovered_keys)?;
        eval::discover(area, &mut discovered_keys)?;
        info!(%user_name, "discovered {:?}", discovered_keys);

        let reply = action.perform((&world, &user, &area))?;

        info!(%user_name, "done {:?}", reply);

        Ok(reply)
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
    storage_factory: Box<dyn EntityStorageFactory>,
}

impl Domain {
    pub fn new(storage_factory: Box<dyn EntityStorageFactory>) -> Self {
        info!("domain-new");

        Domain {
            storage_factory: storage_factory,
        }
    }

    pub fn open_session(&self) -> Result<Session> {
        info!("session-open");

        // TODO Consider using factory in Domain.
        let storage = self.storage_factory.create_storage()?;

        let session = Session::new(storage);

        // TODO get user
        // TODO get Area
        // TODO discover
        // TODO hydrate

        Ok(session)
    }
}
