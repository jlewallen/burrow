use anyhow::Result;
use std::{
    env,
    rc::Rc,
    sync::atomic::{AtomicBool, AtomicI64, Ordering},
};
use tracing::{debug, event, info, span, trace, warn, Level};

use super::internal::{DomainInfrastructure, EntityMap, LoadedEntity};
use crate::plugins::{identifiers, moving::model::Occupying, users::model::Usernames};
use crate::storage::{EntityStorage, EntityStorageFactory, PersistedEntity};
use crate::{kernel::*, plugins::eval};

#[derive(Debug)]
pub struct GlobalIds {
    gid: AtomicI64,
}

impl GlobalIds {
    pub fn new() -> Rc<Self> {
        Rc::new(Self {
            gid: AtomicI64::new(0),
        })
    }

    pub fn gid(&self) -> i64 {
        self.gid.load(Ordering::Relaxed)
    }
}

impl GeneratesGlobalIdentifiers for GlobalIds {
    fn generate_gid(&self) -> Result<i64> {
        // If this is ever used in a multithreaded context, this should be
        // improved upon. For now, this is only used in single threaded
        // situations, we rely on the database for the rest.
        let id = self.gid.load(Ordering::Relaxed) + 1;
        self.gid.store(id, Ordering::Relaxed);
        Ok(id)
    }
}

pub struct Session {
    infra: Rc<DomainInfrastructure>,
    storage: Rc<dyn EntityStorage>,
    entity_map: Rc<EntityMap>,
    open: AtomicBool,
    discoverying: bool,
    global_ids: Rc<GlobalIds>,
}

impl Session {
    pub fn new(storage: Rc<dyn EntityStorage>) -> Result<Self> {
        info!("session-new");

        let entity_map = EntityMap::new();
        let global_ids = GlobalIds::new();

        let generates_ids = Rc::clone(&global_ids) as Rc<dyn GeneratesGlobalIdentifiers>;

        Ok(Self {
            infra: DomainInfrastructure::new(
                Rc::clone(&storage),
                Rc::clone(&entity_map),
                Rc::clone(&generates_ids),
            ),
            storage,
            entity_map,
            open: AtomicBool::new(true),
            discoverying: true,
            global_ids,
        })
    }

    pub fn infra(&self) -> Rc<dyn Infrastructure> {
        self.infra.clone() as Rc<dyn Infrastructure>
    }

    pub fn evaluate_and_perform(
        &self,
        user_name: &str,
        text: &str,
    ) -> Result<Option<Box<dyn Reply>>> {
        if !self.open.load(Ordering::Relaxed) {
            return Err(DomainError::SessionClosed.into());
        }

        match self.perform(user_name, text) {
            Ok(i) => Ok(i),
            Err(original_err) => {
                if let Err(_rollback_err) = self.storage.rollback(false) {
                    panic!("error rolling back");
                }

                self.open.store(false, Ordering::Relaxed);

                Err(original_err)
            }
        }
    }

    fn maybe_save_gid(&self) -> Result<()> {
        let world = self
            .infra
            .load_entity_by_key(&WORLD_KEY)?
            .ok_or(DomainError::EntityNotFound)?;
        let previous_gid = identifiers::model::get_gid(&world)?.unwrap_or(0);
        let new_gid = self.global_ids.gid();
        if previous_gid != new_gid {
            info!("gid:changed {} -> {}", previous_gid, new_gid);
            identifiers::model::set_gid(&world, new_gid)?;
        } else {
            info!("gid:same {}", previous_gid);
        }
        Ok(())
    }

    pub fn flush(&self) -> Result<()> {
        if self.should_flush_entities()? {
            self.maybe_save_gid()?;
            if should_force_rollback() {
                let _span = span!(Level::DEBUG, "FORCED").entered();
                self.storage.rollback(true)
            } else {
                self.storage.commit()
            }
        } else {
            self.storage.rollback(true)
        }
    }

    pub fn close(&self) -> Result<()> {
        self.flush()?;

        self.storage.begin()?;

        self.open.store(false, Ordering::Relaxed);

        Ok(())
    }

    fn evaluate_user_name(&self, user_name: &str) -> Result<(EntityPtr, EntityPtr, EntityPtr)> {
        let _span = span!(Level::DEBUG, "L").entered();

        let world = self
            .infra
            .load_entity_by_key(&WORLD_KEY)?
            .ok_or(DomainError::EntityNotFound)?;

        let usernames: OpenScope<Usernames> = {
            let world = world.borrow();
            world.scope::<Usernames>()?
        };

        let user_key = &usernames.users[user_name];

        let user = self
            .infra
            .load_entity_by_key(user_key)?
            .ok_or(DomainError::EntityNotFound)?;

        let area: EntityPtr = {
            let user = user.borrow();
            let occupying: OpenScope<Occupying> = user.scope::<Occupying>()?;
            occupying.area.into_entity()?
        };

        info!("area {}", area.borrow());

        Ok((world, user, area))
    }

    fn discover_from(&self, entities: Vec<&EntityPtr>) -> Result<Vec<EntityKey>> {
        let _span = span!(Level::DEBUG, "D").entered();
        let mut discovered: Vec<EntityKey> = vec![];
        if self.discoverying {
            for entity in &entities {
                eval::discover(&entity.borrow(), &mut discovered)?;
            }
            info!("discovered {:?}", discovered);
        }
        Ok(discovered)
    }

    fn perform_action(&self, user_name: &str, action: Box<dyn Action>) -> Result<Box<dyn Reply>> {
        info!("performing {:?}", action);

        let (world, user, area) = self.evaluate_user_name(user_name)?;

        self.discover_from(vec![&user, &area])?;

        let reply = {
            let _span = span!(Level::INFO, "A").entered();
            action.perform((world, user, area, self.infra.clone()))?
        };

        event!(Level::INFO, "done");

        Ok(reply)
    }

    fn perform(&self, user_name: &str, text: &str) -> Result<Option<Box<dyn Reply>>> {
        let _doing_span = span!(Level::INFO, "session-do", user = user_name).entered();

        debug!("'{}'", text);

        if let Some(action) = eval::evaluate(text)? {
            Ok(Some(self.perform_action(user_name, action)?))
        } else {
            Ok(None)
        }
    }

    fn check_for_changes(&self, l: &LoadedEntity) -> Result<Option<PersistedEntity>> {
        use treediff::diff;
        use treediff::tools::ChangeType;
        use treediff::tools::Recorder;

        let entity = l.entity.borrow();

        let _span = span!(Level::DEBUG, "flushing", key = entity.key.to_string()).entered();

        let serialized = serde_json::to_string(&*entity)?;

        trace!("json: {:?}", serialized);

        let v1: serde_json::Value = if let Some(serialized) = &l.serialized {
            serialized.parse()?
        } else {
            serde_json::Value::Null
        };
        let v2: serde_json::Value = serialized.parse()?;
        let mut d = Recorder::default();
        diff(&v1, &v2, &mut d);

        let modifications = d
            .calls
            .iter()
            .filter(|c| !matches!(c, ChangeType::Unchanged(_, _)))
            .count();

        if modifications > 0 {
            for each in d.calls {
                match each {
                    ChangeType::Unchanged(_, _) => {}
                    _ => debug!("modified: {:?}", each),
                }
            }

            Ok(Some(PersistedEntity {
                key: entity.key.to_string(),
                gid: l.gid.clone().into(),
                version: l.version,
                serialized,
            }))
        } else {
            Ok(None)
        }
    }

    fn get_modified_entities(&self) -> Result<Vec<PersistedEntity>> {
        let saved = self
            .entity_map
            .foreach_entity(|l| self.check_for_changes(l))?;
        Ok(saved.into_iter().flatten().collect::<Vec<_>>())
    }

    fn should_flush_entities(&self) -> Result<bool> {
        Ok(!self
            .get_modified_entities()?
            .into_iter()
            .map(|p| self.storage.save(&p))
            .collect::<Result<Vec<_>>>()?
            .is_empty())
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if self.open.load(Ordering::Relaxed) {
            warn!("session-drop: open session!");
        } else {
            trace!("session-drop");
        }
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

        storage.begin()?;

        Session::new(storage)
    }
}

fn should_force_rollback() -> bool {
    env::var("FORCE_ROLLBACK").is_ok()
}
