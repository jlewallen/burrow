use anyhow::{anyhow, Result};
use std::{cell::RefCell, rc::Rc, sync::Arc};
use tracing::*;

use super::internal::{Entities, LoadedEntity};
use crate::{
    storage::{PersistedEntity, PersistedFuture, Storage},
    Notifier,
};
use kernel::*;

#[derive(Default)]
pub struct State {
    entities: Rc<Entities>,
    raised: Rc<RefCell<Vec<Raised>>>,
    futures: Rc<RefCell<Vec<Scheduling>>>,
    destroyed: RefCell<Vec<EntityKey>>,
}

impl State {
    pub fn obliterate(&self, entry: &Entry) -> Result<()> {
        let destroying = entry.entity();
        let mut destroying = destroying.borrow_mut();
        destroying.destroy()?;

        self.destroyed.borrow_mut().push(entry.key().clone());

        Ok(())
    }

    pub fn close<T: Notifier>(
        &self,
        storage: &Rc<dyn Storage>,
        notifier: &T,
        finder: &Arc<dyn Finder>,
    ) -> Result<bool> {
        let entities_changed = self.flush_entities(storage)?;
        let raised_changed = self.flush_raised(notifier, finder)?;
        let futures_changed = self.flush_futures(storage)?;
        Ok(entities_changed || raised_changed || futures_changed)
    }

    pub fn size(&self) -> usize {
        self.entities.size()
    }

    pub fn lookup_entity(&self, lookup: &LookupBy) -> Result<Option<EntityPtr>> {
        self.entities.lookup_entity(lookup)
    }

    pub fn add_persisted(&self, persisted: PersistedEntity) -> Result<EntityPtr> {
        self.entities.add_persisted(persisted)
    }

    pub fn add_entity(&self, gid: EntityGid, entity: &EntityPtr) -> Result<()> {
        self.entities.add_entity(gid, entity)
    }

    fn flush_entities(&self, storage: &Rc<dyn Storage>) -> Result<bool> {
        let mut destroyed = self.destroyed.borrow_mut();
        let saves = SavesEntities {
            storage,
            destroyed: &destroyed,
        };
        let changes = saves.save_modified_entities(&self.entities)?;

        destroyed.clear();

        Ok(changes)
    }

    fn flush_raised<T: Notifier>(&self, notifier: &T, finder: &Arc<dyn Finder>) -> Result<bool> {
        let mut pending = self.raised.borrow_mut();
        if pending.is_empty() {
            return Ok(false);
        }

        info!(pending = %pending.len(), "raising");

        for raised in pending.iter() {
            debug!("{:?}", raised.event);
            debug!("{:?}", raised.event.to_tagged_json()?);
            let audience_keys = finder.find_audience(&raised.audience)?;
            for key in audience_keys {
                notifier.notify(&key, &raised.event)?;
            }
        }

        pending.clear();

        Ok(true)
    }

    fn flush_futures(&self, storage: &Rc<dyn Storage>) -> Result<bool> {
        let mut futures = self.futures.borrow_mut();
        if futures.is_empty() {
            return Ok(false);
        }

        for future in futures.iter() {
            storage.queue(PersistedFuture {
                key: future.key.clone(),
                time: future.when.to_utc_time()?,
                serialized: future.message.to_string(),
            })?;
        }

        futures.clear();

        Ok(true)
    }

    fn queue_raised(&self, raised: Raised) -> Result<()> {
        info!("{:?}", raised);

        self.raised.borrow_mut().push(raised);

        Ok(())
    }

    fn queue_scheduled(&self, scheduling: Scheduling) -> Result<()> {
        info!("{:?}", scheduling);

        self.futures.borrow_mut().push(scheduling);

        Ok(())
    }
}

impl Performer for State {
    fn perform(&self, perform: Perform) -> Result<Effect> {
        match perform {
            Perform::Surroundings {
                surroundings,
                action,
            } => match action {
                PerformAction::Instance(action) => {
                    let _span = span!(Level::DEBUG, "A").entered();
                    info!("action:perform {:?}", &action);
                    let res = action.perform(get_my_session()?, &surroundings);
                    if let Ok(effect) = &res {
                        trace!("action:effect {:?}", effect);
                        info!("action:effect");
                    } else {
                        warn!("action:error {:?}", res);
                    }
                    res
                }
            },
            Perform::Raised(raised) => {
                self.queue_raised(raised)?;

                Ok(Effect::Ok)
            }
            Perform::Schedule(scheduling) => {
                self.queue_scheduled(scheduling)?;

                Ok(Effect::Ok)
            }
            _ => todo!(),
        }
    }
}

pub struct ModifiedEntity(PersistedEntity);

pub struct SavesEntities<'a> {
    pub storage: &'a Rc<dyn Storage>,
    pub destroyed: &'a Vec<EntityKey>,
}

impl<'a> SavesEntities<'a> {
    fn check_for_changes(&self, l: &mut LoadedEntity) -> Result<Option<ModifiedEntity>> {
        use kernel::compare::*;

        let _span = span!(Level::INFO, "flushing", key = l.key.to_string()).entered();

        if let Some(modified) = any_entity_changes(AnyChanges {
            before: l.serialized.as_ref().map(Original::String),
            after: l.entity.clone(),
        })? {
            // Serialize to string now that we know we'll use this.
            let serialized = modified.after.to_string();

            // By now we should have a global identifier.
            let Some(gid) = l.gid.clone() else  {
                return Err(anyhow!("Expected EntityGid in check_for_changes"));
            };

            let previous = l.version;
            l.version += 1;

            Ok(Some(ModifiedEntity(PersistedEntity {
                key: l.key.to_string(),
                gid: gid.into(),
                version: previous,
                serialized,
            })))
        } else {
            Ok(None)
        }
    }

    fn save_entity(&self, modified: &ModifiedEntity) -> Result<()> {
        if self.is_deleted(&EntityKey::new(&modified.0.key)) {
            self.storage.delete(&modified.0)
        } else {
            self.storage.save(&modified.0)
        }
    }

    fn is_deleted(&self, key: &EntityKey) -> bool {
        self.destroyed.contains(key)
    }

    pub fn save_modified_entities(&self, entities: &Entities) -> Result<bool> {
        Ok(!self
            .get_modified_entities(entities)?
            .into_iter()
            .map(|modified| self.save_entity(&modified))
            .collect::<Result<Vec<_>>>()?
            .is_empty())
    }

    fn get_modified_entities(&self, entities: &Entities) -> Result<Vec<ModifiedEntity>> {
        let modified = entities.foreach_entity_mut(|l| self.check_for_changes(l))?;
        Ok(modified.into_iter().flatten().collect::<Vec<_>>())
    }
}

#[allow(dead_code)]
pub struct ActionPerformer {
    session: SessionRef,
    surroundings: Surroundings,
    // action: Rc<dyn Action>,
}

#[allow(unused_variables)]
impl Performer for ActionPerformer {
    fn perform(&self, perform: Perform) -> Result<Effect> {
        match perform {
            Perform::Living { living, action } => todo!(),
            Perform::Surroundings {
                surroundings,
                action,
            } => todo!(),
            Perform::Chain(_) => todo!(),
            Perform::Delivery(_) => todo!(),
            Perform::Raised(_) => todo!(),
            Perform::Schedule(_) => todo!(),
            Perform::Ping(_) => todo!(),
            _ => todo!(),
        }
    }
}
