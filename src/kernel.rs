use crate::eval;
use anyhow::Result;
use std::collections::HashMap;
use thiserror::Error;
use tracing::{debug, info};

pub trait Action {
    fn perform(&self) -> Result<()>;
}

#[derive(Error, Debug)]
pub enum EvaluationError {
    #[error("unknown parsing human readable")]
    ParseError,
}

pub type EntityKey = String;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Entity {
    pub key: EntityKey,
}

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EntityRef {
    pub key: EntityKey,
}

pub trait DomainEvent {}

pub trait Scope {}

#[derive(Debug)]
pub struct DomainResult<T> {
    pub events: Vec<T>,
}

pub struct Session {
    storage: Box<dyn EntityStorage>,
    entities: HashMap<EntityKey, Entity>,
}

impl Session {
    pub fn evaluate_and_perform(&self, text: &str) -> Result<()> {
        debug!("session-do '{}'", text);

        let action = eval::evaluate(text)?;
        let _performed = action.perform()?;

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

pub trait EntityStorage {
    fn load(&self, key: &EntityKey) -> Result<Entity>;
    fn save(&self, key: &EntityKey, entity: &Entity) -> Result<()>;
}

pub mod sqlite {
    use super::*;
    use anyhow::anyhow;
    use rusqlite::Connection;

    pub struct SqliteStorage {
        path: String,
    }

    impl SqliteStorage {
        pub fn new(path: &str) -> Self {
            SqliteStorage {
                path: path.to_string(),
            }
        }
    }

    #[derive(Debug)]
    struct PersistedEntity {
        key: String,
        gid: u32,
        version: u32,
        serialized: String,
    }

    impl PersistedEntity {
        fn to_entity(&self) -> Result<Entity> {
            let entity: Entity = serde_json::from_str(&self.serialized)?;

            info!(%entity.key, "parsed");

            return Ok(entity);
        }
    }

    impl EntityStorage for SqliteStorage {
        fn load(&self, key: &EntityKey) -> Result<Entity> {
            let conn = Connection::open(&self.path)?;

            let mut stmt =
                conn.prepare("SELECT key, gid, version, serialized FROM entities WHERE key = ?;")?;

            debug!(%key, "querying");

            let mut entities = stmt.query_map([key], |row| {
                Ok(PersistedEntity {
                    key: row.get(0)?,
                    gid: row.get(1)?,
                    version: row.get(2)?,
                    serialized: row.get(3)?,
                })
            })?;

            match entities.next() {
                Some(p) => p?.to_entity(),
                _ => Err(anyhow!("entity with key {} not found", key)),
            }
        }

        fn save(&self, _key: &EntityKey, _entity: &Entity) -> Result<()> {
            unimplemented!()
        }
    }
}

pub struct Domain {
    // storage: Box<dyn EntityStorage>,
}

impl Domain {
    pub fn new() -> Self {
        info!("domain-new");

        Domain {
            // TODO Consider making this a factory.
            // storage: Box::new(sqlite::SqliteStorage::new("world.sqlite3")),
        }
    }

    pub fn open_session(&self) -> Result<Session> {
        info!("session-open");

        // TODO Consider using factory in Domain.
        let storage = Box::new(sqlite::SqliteStorage::new("world.sqlite3"));

        let world_key = "world".to_string();

        storage.load(&world_key)?;

        Ok(Session {
            storage: storage,
            entities: HashMap::new(),
        })
    }
}
