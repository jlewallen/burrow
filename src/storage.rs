use crate::kernel::*;
use anyhow::Result;
use tracing::{debug, info};

pub trait EntityStorage {
    fn load(&self, key: &EntityKey) -> Result<Entity>;
    fn save(&self, key: &EntityKey, entity: &Entity) -> Result<()>;
}

pub trait EntityStorageFactory {
    fn create_storage(&self) -> Result<Box<dyn EntityStorage>>;
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
    #[allow(dead_code)]
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

    pub struct Factory {
        path: String,
    }

    impl Factory {
        pub fn new(path: &str) -> Box<Factory> {
            Box::new(Factory {
                path: path.to_string(),
            })
        }
    }

    impl EntityStorageFactory for Factory {
        fn create_storage(&self) -> Result<Box<dyn EntityStorage>> {
            Ok(Box::new(SqliteStorage::new(&self.path)))
        }
    }
}
