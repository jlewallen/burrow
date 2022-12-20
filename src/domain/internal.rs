use anyhow::{anyhow, Result};
use std::sync::atomic::{AtomicU64, Ordering};
use std::{cell::RefCell, collections::HashMap, fmt::Debug, rc::Rc};
use tracing::*;

use crate::kernel::*;
use crate::storage::{EntityStorage, PersistedEntity};

#[derive(Debug)]
pub struct LoadedEntity {
    pub key: EntityKey,
    pub entity: EntityPtr,
    pub version: u64,
    pub gid: Option<EntityGid>,
    pub serialized: Option<String>,
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

    fn lookup_entity_by_key(&self, key: &EntityKey) -> Result<Option<EntityPtr>> {
        if let Some(e) = self.by_key.get(key) {
            trace!(%key, "existing");
            Ok(Some(e.entity.clone()))
        } else {
            Ok(None)
        }
    }

    fn lookup_entity_by_gid(&self, gid: &EntityGid) -> Result<Option<EntityPtr>> {
        if let Some(k) = self.by_gid.get(gid) {
            Ok(self.lookup_entity_by_key(k)?)
        } else {
            Ok(None)
        }
    }

    fn add_entity(&mut self, loaded: LoadedEntity) -> Result<()> {
        info!("adding {:?}", loaded);

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

    pub fn lookup_entity_by_key(&self, key: &EntityKey) -> Result<Option<EntityPtr>> {
        self.maps.borrow().lookup_entity_by_key(key)
    }

    pub fn lookup_entity_by_gid(&self, gid: &EntityGid) -> Result<Option<EntityPtr>> {
        self.maps.borrow().lookup_entity_by_gid(gid)
    }

    pub fn add_entity(&self, mut loaded: LoadedEntity) -> Result<()> {
        self.assign_gid_if_necessary(&mut loaded);
        self.maps.borrow_mut().add_entity(loaded)
    }

    fn assign_gid_if_necessary(&self, mut loaded: &mut LoadedEntity) {
        if loaded.gid.is_none() {
            let loaded = &mut loaded;
            loaded.gid = Some(self.ids.get());
        }
    }

    pub fn foreach_entity_mut<R, T: Fn(&mut LoadedEntity) -> Result<R>>(
        &self,
        each: T,
    ) -> Result<Vec<R>> {
        self.maps.borrow_mut().foreach_entity_mut(each)
    }

    #[allow(dead_code)]
    pub fn foreach_entity<R, T: Fn(&LoadedEntity) -> Result<R>>(&self, each: T) -> Result<Vec<R>> {
        self.maps.borrow().foreach_entity(each)
    }
}

pub struct Entities {
    entities: Rc<EntityMap>,
    storage: Rc<dyn EntityStorage>,
}

impl Entities {
    pub fn new(entities: Rc<EntityMap>, storage: Rc<dyn EntityStorage>) -> Rc<Self> {
        Rc::new(Self { entities, storage })
    }

    pub fn add_entity(&self, entity: &EntityPtr) -> Result<()> {
        let clone = entity.clone();
        let entity = entity.borrow();
        self.entities.add_entity(LoadedEntity {
            key: entity.key.clone(),
            entity: clone,
            serialized: None,
            version: 1,
            gid: entity.gid(),
        })
    }

    fn prepare_persisted(&self, persisted: PersistedEntity) -> Result<EntityPtr> {
        trace!("parsing");
        let mut loaded: Entity = serde_json::from_str(&persisted.serialized)?;

        trace!("infrastructure");
        let session = get_my_session()?; // Thread local session!
        loaded.supply(&session)?;

        let gid = loaded.gid();
        let cell: EntityPtr = loaded.into();

        self.entities.add_entity(LoadedEntity {
            key: EntityKey::new(&persisted.key),
            entity: cell.clone(),
            version: persisted.version + 1,
            gid,
            serialized: Some(persisted.serialized),
        })?;

        Ok(cell)
    }

    pub fn prepare_entity_by_key(&self, key: &EntityKey) -> Result<Option<EntityPtr>> {
        if let Some(e) = self.entities.lookup_entity_by_key(key)? {
            return Ok(Some(e));
        }

        let _loading_span = span!(Level::INFO, "entity", key = key.key_to_string()).entered();

        info!("loading");
        if let Some(persisted) = self.storage.load_by_key(key)? {
            Ok(Some(self.prepare_persisted(persisted)?))
        } else {
            Ok(None)
        }
    }

    pub fn prepare_entity_by_gid(&self, gid: &EntityGid) -> Result<Option<EntityPtr>> {
        if let Some(e) = self.entities.lookup_entity_by_gid(gid)? {
            return Ok(Some(e));
        }

        let _loading_span = span!(Level::INFO, "entity", gid = gid.gid_to_string()).entered();

        info!("loading");
        if let Some(persisted) = self.storage.load_by_gid(gid)? {
            Ok(Some(self.prepare_persisted(persisted)?))
        } else {
            Ok(None)
        }
    }
}

#[derive(Debug)]
pub struct GlobalIds {
    gid: AtomicU64,
}

impl GlobalIds {
    pub fn new() -> Rc<Self> {
        Rc::new(Self {
            gid: AtomicU64::new(0),
        })
    }

    pub fn gid(&self) -> EntityGid {
        EntityGid::new(self.gid.load(Ordering::Relaxed))
    }

    pub fn set(&self, gid: &EntityGid) {
        self.gid.store(gid.into(), Ordering::Relaxed);
    }

    pub fn get(&self) -> EntityGid {
        EntityGid::new(self.gid.fetch_add(1, Ordering::Relaxed) + 1)
    }
}
