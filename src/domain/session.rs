use anyhow::Result;
use std::{
    rc::Rc,
    sync::atomic::{AtomicBool, Ordering},
};
use tracing::{debug, event, info, span, trace, warn, Level};

use super::eval;
use super::internal::{DomainInfrastructure, EntityMap};
use crate::kernel::*;
use crate::plugins::{moving::model::Occupying, users::model::Usernames};
use crate::storage::{EntityStorage, EntityStorageFactory, PersistedEntity};

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
            storage: storage,
            entity_map: entity_map,
            open: AtomicBool::new(true),
            discoverying: true,
        })
    }

    pub fn evaluate_and_perform(&self, user_name: &str, text: &str) -> Result<Box<dyn Reply>> {
        if !self.open.load(Ordering::Relaxed) {
            return Err(DomainError::SessionClosed.into());
        }

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
                if let Err(_rollback_err) = self.storage.rollback(false) {
                    panic!("error rolling back");
                }

                self.open.store(false, Ordering::Relaxed);

                Err(original_err)
            }
        }
    }

    pub fn close(&self) -> Result<()> {
        use treediff::diff;
        use treediff::tools::ChangeType;
        use treediff::tools::Recorder;

        let saved = self.entity_map.foreach_entity(|l| {
            let entity = l.entity.borrow();

            let _span = span!(Level::DEBUG, "saving", key = entity.key.to_string()).entered();

            let serialized = serde_json::to_string(&*entity)?;
            let v1: serde_json::Value = l.serialized.parse()?;
            let v2: serde_json::Value = serialized.parse()?;

            trace!("foreach: {:?}", serialized);

            let mut d = Recorder::default();

            diff(&v1, &v2, &mut d);

            let modifications = d
                .calls
                .iter()
                .filter(|c| match c {
                    ChangeType::Modified(_, _, _) => true,
                    _ => false,
                })
                .count();

            if modifications > 0 {
                for each in d.calls {
                    match each {
                        ChangeType::Unchanged(_, _) => {}
                        _ => debug!("modified: {:?}", each),
                    }
                }

                self.storage.save(&PersistedEntity {
                    key: entity.key.to_string(),
                    gid: l.gid,
                    version: l.version,
                    serialized,
                })?;
            }

            Ok(modifications > 0)
        })?;

        if saved.iter().any(|p| *p) {
            self.storage.commit()?;
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

        let _ = storage.begin()?;

        Session::new(storage)
    }
}
