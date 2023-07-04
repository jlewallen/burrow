use anyhow::Result;
use chrono::{DateTime, Utc};
use std::{
    collections::HashMap,
    rc::Rc,
    sync::{Arc, RwLock},
};

use kernel::{EntityGid, EntityKey, LookupBy};

pub trait EntityStorage: FutureStorage {
    fn load(&self, lookup: &LookupBy) -> Result<Option<PersistedEntity>>;
    fn save(&self, entity: &PersistedEntity) -> Result<()>;
    fn delete(&self, entity: &PersistedEntity) -> Result<()>;
    fn begin(&self) -> Result<()>;
    fn rollback(&self, benign: bool) -> Result<()>;
    fn commit(&self) -> Result<()>;
    fn query_all(&self) -> Result<Vec<PersistedEntity>>;
}

pub trait EntityStorageFactory: Send + Sync {
    fn create_storage(&self) -> Result<Rc<dyn EntityStorage>>;
}

#[derive(Debug, Clone)]
pub struct PersistedFuture {
    pub key: String,
    pub time: chrono::DateTime<chrono::Utc>,
    pub serialized: String,
}

pub trait FutureStorage {
    fn queue(&self, future: PersistedFuture) -> Result<()>;
    fn cancel(&self, key: &str) -> Result<()>;
    fn query_futures_before(&self, now: DateTime<Utc>) -> Result<Vec<PersistedFuture>>;
}

#[derive(Clone, Debug)]
pub struct PersistedEntity {
    pub key: String,
    pub gid: u64,
    pub version: u64,
    pub serialized: String,
}

impl PersistedEntity {
    pub fn to_json_value(&self) -> Result<serde_json::Value> {
        Ok(serde_json::from_str(&self.serialized)?)
    }
}

#[derive(Default)]
pub struct InMemoryEntityStorageFactory {
    entities: Arc<RwLock<HashMap<EntityKey, PersistedEntity>>>,
}

impl EntityStorageFactory for InMemoryEntityStorageFactory {
    fn create_storage(&self) -> Result<Rc<dyn EntityStorage>> {
        Ok(Rc::new(InMemoryEntityStorage {
            entities: self.entities.clone(),
            pending: Default::default(),
            futures: Default::default(),
        }))
    }
}

enum Pending {
    Save(PersistedEntity),
    Delete(PersistedEntity),
}

pub struct InMemoryEntityStorage {
    entities: Arc<RwLock<HashMap<EntityKey, PersistedEntity>>>,
    futures: Arc<RwLock<HashMap<String, PersistedFuture>>>,
    pending: RwLock<Vec<Pending>>,
}

impl FutureStorage for InMemoryEntityStorage {
    fn queue(&self, future: PersistedFuture) -> Result<()> {
        let mut futures = self.futures.write().expect("Lock error");
        futures.insert(future.key.clone(), future);

        Ok(())
    }

    fn cancel(&self, key: &str) -> Result<()> {
        let mut futures = self.futures.write().expect("Lock error");
        futures.remove(key);

        Ok(())
    }

    fn query_futures_before(&self, now: DateTime<Utc>) -> Result<Vec<PersistedFuture>> {
        let mut futures = self.futures.write().expect("Lock error");
        let mut pending = Vec::new();

        for (_k, future) in futures.iter() {
            if now >= future.time {
                pending.push(future.clone());
            }
        }

        for future in pending.iter() {
            futures.remove(&future.key);
        }

        Ok(pending)
    }
}

impl EntityStorage for InMemoryEntityStorage {
    fn load(&self, lookup: &LookupBy) -> Result<Option<PersistedEntity>> {
        let entities = self.entities.read().expect("Lock error");
        let entity = entities
            .iter()
            .filter(|(_, e)| match lookup {
                LookupBy::Key(key) => e.key == key.key_to_string(),
                LookupBy::Gid(gid) => EntityGid::new(e.gid) == **gid,
            })
            .map(|(_, e)| e)
            .next();

        Ok(entity.cloned())
    }

    fn save(&self, entity: &PersistedEntity) -> Result<()> {
        let mut pending = self.pending.write().expect("Lock error");
        pending.push(Pending::Save(entity.clone()));

        Ok(())
    }

    fn delete(&self, entity: &PersistedEntity) -> Result<()> {
        let mut pending = self.pending.write().expect("Lock error");
        pending.push(Pending::Delete(entity.clone()));

        Ok(())
    }

    fn begin(&self) -> Result<()> {
        let mut pending = self.pending.write().expect("Lock error");
        pending.clear();

        Ok(())
    }

    fn rollback(&self, _benign: bool) -> Result<()> {
        let mut pending = self.pending.write().expect("Lock error");
        pending.clear();

        Ok(())
    }

    fn commit(&self) -> Result<()> {
        let mut pending = self.pending.write().expect("Lock error");
        let mut entities = self.entities.write().expect("Lock error");

        for pending in pending.iter() {
            match pending {
                Pending::Save(e) => entities.insert(EntityKey::new(&e.key), e.clone()),
                Pending::Delete(e) => entities.remove(&EntityKey::new(&e.key)),
            };
        }

        pending.clear();

        Ok(())
    }

    fn query_all(&self) -> Result<Vec<PersistedEntity>> {
        let entities = self.entities.read().expect("Lock error");

        Ok(entities.values().map(|e| e.clone()).collect())
    }
}
