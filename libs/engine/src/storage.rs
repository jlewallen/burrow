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

pub trait Storage: EntityStorage + FutureStorage {}

pub trait StorageFactory: Send + Sync {
    fn migrate(&self) -> Result<()>;

    fn create_storage(&self) -> Result<Rc<dyn Storage>>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistedFuture {
    pub key: String,
    pub time: chrono::DateTime<chrono::Utc>,
    pub serialized: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingFutures {
    Futures(Vec<PersistedFuture>),
    Waiting(Option<DateTime<Utc>>),
}

impl PendingFutures {
    pub fn number_futures(&self) -> Option<usize> {
        match self {
            PendingFutures::Futures(futures) => Some(futures.len()),
            PendingFutures::Waiting(_) => None,
        }
    }
}

pub trait FutureStorage {
    fn queue(&self, future: PersistedFuture) -> Result<()>;
    fn cancel(&self, key: &str) -> Result<()>;
    fn query_futures_before(&self, now: DateTime<Utc>) -> Result<PendingFutures>;
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
pub struct InMemoryStorageFactory {
    entities: Arc<RwLock<HashMap<EntityKey, PersistedEntity>>>,
}

impl StorageFactory for InMemoryStorageFactory {
    fn migrate(&self) -> Result<()> {
        Ok(())
    }

    fn create_storage(&self) -> Result<Rc<dyn Storage>> {
        Ok(Rc::new(InMemoryStorage {
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

pub struct InMemoryStorage {
    entities: Arc<RwLock<HashMap<EntityKey, PersistedEntity>>>,
    futures: Arc<RwLock<HashMap<String, PersistedFuture>>>,
    pending: RwLock<Vec<Pending>>,
}

impl Storage for InMemoryStorage {}

impl FutureStorage for InMemoryStorage {
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

    fn query_futures_before(&self, now: DateTime<Utc>) -> Result<PendingFutures> {
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

        if pending.is_empty() {
            Ok(PendingFutures::Waiting(None))
        } else {
            Ok(PendingFutures::Futures(pending))
        }
    }
}

impl EntityStorage for InMemoryStorage {
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
