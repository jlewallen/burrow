use anyhow::{anyhow, Result};
use std::{cell::RefCell, collections::HashMap, fmt::Debug, rc::Rc};
use tracing::*;

use super::sequences::GlobalIds;
use crate::storage::PersistedEntity;
use kernel::*;

pub struct LoadedEntity {
    pub key: EntityKey,
    pub entity: EntityPtr,
    pub version: u64,
    pub gid: Option<EntityGid>,
    pub serialized: Option<String>,
}

impl Debug for LoadedEntity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoadedEntity")
            .field("entity", &self.entity)
            .finish()
    }
}

#[derive(Default)]
struct Maps {
    by_key: HashMap<EntityKey, LoadedEntity>,
    by_gid: HashMap<EntityGid, EntityKey>,
}

impl Maps {
    fn size(&self) -> usize {
        self.by_key.len()
    }

    fn lookup_entity(&self, lookup: &LookupBy) -> Result<Option<EntityPtr>> {
        match lookup {
            LookupBy::Key(key) => {
                if let Some(e) = self.by_key.get(key) {
                    trace!(%key, "existing");
                    Ok(Some(e.entity.clone()))
                } else {
                    Ok(None)
                }
            }
            LookupBy::Gid(gid) => {
                if let Some(k) = self.by_gid.get(gid) {
                    Ok(self.lookup_entity(&LookupBy::Key(k))?)
                } else {
                    Ok(None)
                }
            }
        }
    }

    fn add_entity(&mut self, loaded: LoadedEntity) -> Result<()> {
        debug!("adding {:?}", loaded);

        self.by_gid.insert(
            loaded
                .gid
                .clone()
                .ok_or_else(|| anyhow!("Entity missing GID"))?,
            loaded.key.clone(),
        );
        self.by_key.insert(loaded.key.clone(), loaded);

        Ok(())
    }

    fn foreach_entity_mut<R, T: Fn(&mut LoadedEntity) -> Result<R>>(
        &mut self,
        each: T,
    ) -> Result<Vec<R>> {
        let mut rvals: Vec<R> = Vec::new();

        for (_key, entity) in self.by_key.iter_mut() {
            rvals.push(each(entity)?);
        }

        Ok(rvals)
    }

    #[cfg(test)]
    fn foreach_entity<R, T: Fn(&LoadedEntity) -> Result<R>>(&self, each: T) -> Result<Vec<R>> {
        let mut rvals: Vec<R> = Vec::new();

        for (_key, entity) in self.by_key.iter() {
            rvals.push(each(entity)?);
        }

        Ok(rvals)
    }
}

#[derive(Default)]
pub struct EntityMap {
    maps: RefCell<Maps>,
}

impl EntityMap {
    pub fn size(&self) -> usize {
        self.maps.borrow().size()
    }

    pub fn lookup_entity(&self, lookup: &LookupBy) -> Result<Option<EntityPtr>> {
        self.maps.borrow().lookup_entity(lookup)
    }

    pub fn add_entity(&self, loaded: LoadedEntity) -> Result<()> {
        self.maps.borrow_mut().add_entity(loaded)
    }

    fn foreach_entity_mut<R, T: Fn(&mut LoadedEntity) -> Result<R>>(
        &self,
        each: T,
    ) -> Result<Vec<R>> {
        self.maps.borrow_mut().foreach_entity_mut(each)
    }

    #[cfg(test)]
    #[allow(dead_code)]
    fn foreach_entity<R, T: Fn(&LoadedEntity) -> Result<R>>(&self, each: T) -> Result<Vec<R>> {
        self.maps.borrow().foreach_entity(each)
    }
}

pub trait AssignEntityId {
    fn assign(&self, entity: &EntityPtr) -> Result<(EntityKey, EntityGid)>;
}

impl AssignEntityId for GlobalIds {
    fn assign(&self, entity: &EntityPtr) -> Result<(EntityKey, EntityGid)> {
        let mut entity = entity.borrow_mut();
        let key = entity.key().clone();
        let gid = entity.gid();
        // Entities should never be added with an existing gid, how would
        // the creator know the value to assign? This is happening, though.
        // assert!(existing.is_none());
        match gid {
            Some(gid) => {
                warn!(%gid, %key, "already has gid");
                Ok((key, gid))
            }
            None => {
                let gid = self.get();
                info!(%key, %gid, "assigning gid");
                entity.set_gid(gid.clone())?;
                Ok((key, gid))
            }
        }
    }
}

#[derive(Default)]
pub struct Entities {
    entities: Rc<EntityMap>,
}

impl Entities {
    pub fn add_entity(&self, ids: &GlobalIds, entity: &EntityPtr) -> Result<()> {
        let clone = entity.clone();
        let (key, gid) = ids.assign(entity)?;
        self.entities.add_entity(LoadedEntity {
            key,
            entity: clone,
            serialized: None,
            version: 1,
            gid: Some(gid),
        })
    }

    pub fn add_persisted(&self, persisted: PersistedEntity) -> Result<EntityPtr> {
        let loaded: Entity = deserialize_entity(&persisted.serialized)?;

        // Verify consistency between serialized Entity gid and the gid on the
        // row. We can eventually relax this.
        let gid: EntityGid = loaded
            .gid()
            .ok_or_else(|| anyhow!("Persisted entities should have gid."))?;
        assert!(
            EntityGid::new(persisted.gid) == gid,
            "Entity gid should match row gid."
        );

        // Wrap entity in memory management gizmos.
        let cell: EntityPtr = loaded.into();

        self.entities.add_entity(LoadedEntity {
            key: EntityKey::new(&persisted.key),
            entity: cell.clone(),
            serialized: Some(persisted.serialized),
            version: persisted.version + 1,
            gid: Some(gid),
        })?;

        Ok(cell)
    }

    pub fn lookup_entity(&self, lookup: &LookupBy) -> Result<Option<EntityPtr>> {
        self.entities.lookup_entity(lookup)
    }

    pub fn foreach_entity_mut<R, T: Fn(&mut LoadedEntity) -> Result<R>>(
        &self,
        each: T,
    ) -> Result<Vec<R>> {
        self.entities.foreach_entity_mut(each)
    }

    pub fn size(&self) -> usize {
        self.entities.size()
    }
}
