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

struct Maps {
    by_key: HashMap<EntityKey, LoadedEntity>,
    by_gid: HashMap<EntityGid, EntityKey>,
}

impl Maps {
    fn new() -> Self {
        Maps {
            by_key: HashMap::new(),
            by_gid: HashMap::new(),
        }
    }

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

pub struct EntityMap {
    ids: Rc<GlobalIds>,
    maps: RefCell<Maps>,
}

impl EntityMap {
    pub fn new(ids: Rc<GlobalIds>) -> Rc<Self> {
        Rc::new(Self {
            ids,
            maps: RefCell::new(Maps::new()),
        })
    }

    pub fn size(&self) -> usize {
        self.maps.borrow().size()
    }

    pub fn lookup_entity(&self, lookup: &LookupBy) -> Result<Option<EntityPtr>> {
        self.maps.borrow().lookup_entity(lookup)
    }

    fn assign_gid_if_necessary(&self, loaded: &mut LoadedEntity) -> Result<()> {
        if loaded.gid.is_none() {
            let gid = self.ids.get();
            info!(%loaded.key, %gid, "entity-map assigning gid");
            loaded.gid = Some(gid.clone());
            loaded.entity.borrow_mut().set_gid(gid)?;
        }

        Ok(())
    }

    pub fn add_entity(&self, mut loaded: LoadedEntity) -> Result<()> {
        self.assign_gid_if_necessary(&mut loaded)?;
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

pub struct Entities {
    entities: Rc<EntityMap>,
}

impl Entities {
    pub fn new(entities: Rc<EntityMap>) -> Rc<Self> {
        Rc::new(Self { entities })
    }

    pub fn add_entity(&self, entity: &EntityPtr) -> Result<()> {
        let clone = entity.clone();
        let (key, gid) = {
            let entity = entity.borrow();
            (entity.key().clone(), entity.gid())
        };
        self.entities.add_entity(LoadedEntity {
            key,
            entity: clone,
            serialized: None,
            version: 1,
            gid,
        })
    }

    pub fn add_persisted(&self, persisted: PersistedEntity) -> Result<EntityPtr> {
        let loaded = deserialize_entity(&persisted.serialized)?;
        let gid = loaded.gid();
        let cell: EntityPtr = loaded.into();

        self.entities.add_entity(LoadedEntity {
            key: EntityKey::new(&persisted.key),
            entity: cell.clone(),
            serialized: Some(persisted.serialized),
            version: persisted.version + 1,
            gid,
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
