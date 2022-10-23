use crate::kernel::EntityKey;
use anyhow::Result;
use tracing::debug;

pub trait EntityStorage {
    fn load(&self, key: &EntityKey) -> Result<PersistedEntity>;
    fn save(&self, entity: &PersistedEntity) -> Result<()>;
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
        conn: Connection,
    }

    impl SqliteStorage {
        pub fn new(path: &str) -> Result<Self> {
            let conn = if path == ":memory:" {
                Connection::open_in_memory()?
            } else {
                Connection::open(&path)?
            };

            {
                let mut stmt = conn.prepare(
                    r#"
                CREATE TABLE IF NOT EXISTS entities (
                    key TEXT NOT NULL PRIMARY KEY,
                    version INTEGER NOT NULL,
                    gid INTEGER,
                    serialized TEXT NOT NULL
                )"#,
                )?;
                let _ = stmt.execute([])?;
            }

            Ok(SqliteStorage { conn: conn })
        }

        /*
        pub fn transanction<T: FnOnce() -> Result<T>>(&self, work: T) -> Result<T> {
            work()
        }
        */
    }

    impl EntityStorage for SqliteStorage {
        fn load(&self, key: &EntityKey) -> Result<PersistedEntity> {
            let mut stmt = self
                .conn
                .prepare("SELECT key, gid, version, serialized FROM entities WHERE key = ?;")?;

            debug!("querying");

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
                _ => Err(anyhow!("entity with key '{}' not found", key)),
            }
        }

        fn save(&self, entity: &PersistedEntity) -> Result<()> {
            let affected = if entity.version == 1 {
                let mut stmt = self.conn.prepare(
                    "INSERT INTO entities (key, gid, version, serialized) VALUES (?1, ?2, ?3, ?4)",
                )?;

                stmt.execute((
                    &entity.key.to_string(),
                    &entity.gid,
                    &entity.version,
                    &entity.serialized,
                ))?
            } else {
                let mut stmt = self.conn.prepare(
                    "UPDATE entities SET gid = ?1, version = ?2, serialized = ?3 WHERE key = ?4 AND version = ?5",
                )?;

                stmt.execute((
                    &entity.gid,
                    &entity.version,
                    &entity.serialized,
                    &entity.key.to_string(),
                    &entity.version - 1,
                ))?
            };

            if affected != 1 {
                Err(anyhow!("no rows affected by save"))
            } else {
                Ok(())
            }
        }
    }

    pub struct Factory {
        path: String,
    }

    impl Factory {
        pub fn new(path: &str) -> Result<Box<Factory>> {
            Ok(Box::new(Factory {
                path: path.to_string(),
            }))
        }
    }

    impl EntityStorageFactory for Factory {
        fn create_storage(&self) -> Result<Box<dyn EntityStorage>> {
            Ok(Box::new(SqliteStorage::new(&self.path)?))
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use anyhow::Result;

        fn get_storage() -> Result<Box<dyn EntityStorage>> {
            let s = Factory::new(":memory:");
            Ok(s.create_storage()?)
        }

        #[test]
        fn it_queries_for_entity_by_missing_key() -> Result<()> {
            let s = get_storage()?;
            match s.load(&EntityKey::new("world")) {
                Ok(_) => Err(anyhow!("unexpected")),
                Err(_e) => {
                    // assert_eq!(e, anyhow!(""));
                    Ok(())
                }
            }
        }

        #[test]
        fn it_inserts_a_new_entity() -> Result<()> {
            let s = get_storage()?;

            s.save(&PersistedEntity {
                key: "world".to_string(),
                gid: 1,
                version: 1,
                serialized: "{}".to_string(),
            })
        }

        #[test]
        fn it_queries_for_entity_by_key() -> Result<()> {
            let s = get_storage()?;

            s.save(&PersistedEntity {
                key: "world".to_string(),
                gid: 1,
                version: 1,
                serialized: "{}".to_string(),
            })?;

            s.load(&EntityKey::new("world"))?;

            Ok(())
        }

        #[test]
        fn it_updates_an_existing_new_entity() -> Result<()> {
            let s = get_storage()?;

            s.save(&PersistedEntity {
                key: "world".to_string(),
                gid: 1,
                version: 1,
                serialized: "{}".to_string(),
            })?;

            let mut p1 = s.load(&EntityKey::new("world"))?;

            assert_eq!(1, p1.version);

            p1.version += 1;

            s.save(&p1)?;

            let p2 = s.load(&EntityKey::new("world"))?;

            assert_eq!(2, p2.version);

            Ok(())
        }
    }
}
