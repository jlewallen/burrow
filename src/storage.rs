use crate::kernel::EntityKey;
use anyhow::Result;
use tracing::debug;

pub trait EntityStorage {
    fn load(&self, key: &EntityKey) -> Result<PersistedEntity>;
    fn save(&self, key: &EntityKey, entity: &PersistedEntity) -> Result<()>;
}

pub trait EntityStorageFactory {
    fn create_storage(&self) -> Result<Box<dyn EntityStorage>>;
}

#[derive(Debug)]
pub struct PersistedEntity {
    pub key: String,
    pub gid: u32,
    pub version: u32,
    pub serialized: String,
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

    impl EntityStorage for SqliteStorage {
        fn load(&self, key: &EntityKey) -> Result<PersistedEntity> {
            let conn = Connection::open(&self.path)?;

            let mut stmt =
                conn.prepare("SELECT key, gid, version, serialized FROM entities WHERE key = ?;")?;

            debug!(%key, "querying");

            let mut entities = stmt.query_map([key.key_to_string()], |row| {
                Ok(PersistedEntity {
                    key: row.get(0)?,
                    gid: row.get(1)?,
                    version: row.get(2)?,
                    serialized: row.get(3)?,
                })
            })?;

            match entities.next() {
                Some(p) => Ok(p?),
                _ => Err(anyhow!("entity with key {} not found", key)),
            }
        }

        fn save(&self, _key: &EntityKey, _entity: &PersistedEntity) -> Result<()> {
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
