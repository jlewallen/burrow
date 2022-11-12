use anyhow::Result;
use std::{
    env,
    rc::Rc,
    sync::atomic::{AtomicBool, Ordering},
};
use tracing::{debug, event, info, span, trace, warn, Level};

use super::internal::{DomainInfrastructure, EntityMap, LoadedEntity};
use crate::plugins::{moving::model::Occupying, users::model::Usernames};
use crate::storage::{EntityStorage, EntityStorageFactory, PersistedEntity};
use crate::{kernel::*, plugins::eval};

pub struct Session {
    infra: Rc<DomainInfrastructure>,
    storage: Rc<dyn EntityStorage>,
    entity_map: Rc<EntityMap>,
    open: AtomicBool,
    discoverying: bool,
}

impl Session {
    pub fn new(storage: Rc<dyn EntityStorage>) -> Result<Self> {
        info!("session-new");

        let entity_map = EntityMap::new();

        Ok(Self {
            infra: DomainInfrastructure::new(Rc::clone(&storage), Rc::clone(&entity_map)),
            storage,
            entity_map,
            open: AtomicBool::new(true),
            discoverying: true,
        })
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

    fn evaluate(&self, user_name: &str) -> Result<(EntityPtr, EntityPtr, EntityPtr)> {
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

        Ok((world, user, area))
    }

    fn perform_action(&self, user_name: &str, action: Box<dyn Action>) -> Result<Box<dyn Reply>> {
        info!("performing {:?}", action);

        let (world, user, area) = self.evaluate(user_name)?;

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

        let v1: serde_json::Value = l.serialized.parse()?;
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

    fn flush_entities(&self) -> Result<bool> {
        Ok(!self
            .get_modified_entities()?
            .into_iter()
            .map(|p| self.storage.save(&p))
            .collect::<Result<Vec<_>>>()?
            .is_empty())
    }

    pub fn close(&self) -> Result<()> {
        if self.flush_entities()? {
            if should_force_rollback() {
                let _span = span!(Level::DEBUG, "FORCED").entered();
                self.storage.rollback(true)?;
            } else {
                self.storage.commit()?;
            }
        } else {
            self.storage.rollback(true)?;
        }

        self.open.store(false, Ordering::Relaxed);

        Ok(())
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
