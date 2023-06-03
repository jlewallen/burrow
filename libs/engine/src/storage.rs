use anyhow::Result;
use std::rc::Rc;
use tracing::*;

use kernel::{EntityGid, EntityKey, LookupBy};

pub trait EntityStorage {
    fn load(&self, lookup: &LookupBy) -> Result<Option<PersistedEntity>>;
    fn save(&self, entity: &PersistedEntity) -> Result<()>;
    fn delete(&self, entity: &PersistedEntity) -> Result<()>;
    fn begin(&self) -> Result<()>;
    fn rollback(&self, benign: bool) -> Result<()>;
    fn commit(&self) -> Result<()>;
}

pub trait EntityStorageFactory: Send + Sync {
    fn create_storage(&self) -> Result<Rc<dyn EntityStorage>>;
}

#[derive(Debug)]
pub struct PersistedEntity {
    pub key: String,
    pub gid: u64,
    pub version: u64,
    pub serialized: String,
}

pub mod sqlite {
    use std::sync::Arc;

    use super::*;
    use anyhow::anyhow;
    use rusqlite::{Connection, OpenFlags};

    pub struct SqliteStorage {
        conn: Connection,
    }

    impl SqliteStorage {
        pub fn new(path: &str) -> Result<Rc<Self>> {
            let conn = if path == ":memory:" {
                Connection::open_with_flags(
                    "file:burrow-1?mode=memory&cache=shared",
                    OpenFlags::SQLITE_OPEN_URI | OpenFlags::SQLITE_OPEN_READ_WRITE,
                )?
            } else {
                Connection::open(path)?
            };

            let exec = |sql: &str| -> Result<usize> {
                let mut stmt = conn.prepare(sql)?;
                Ok(stmt.execute([])?)
            };

            exec(
                r#"
                CREATE TABLE IF NOT EXISTS entities (
                    key TEXT NOT NULL PRIMARY KEY,
                    version INTEGER NOT NULL,
                    gid INTEGER,
                    serialized TEXT NOT NULL
                )"#,
            )?;

            exec(r#"CREATE UNIQUE INDEX IF NOT EXISTS entities_gid ON entities (gid)"#)?;

            Ok(Rc::new(SqliteStorage { conn }))
        }

        fn single_query<T: rusqlite::Params>(
            &self,
            query: &str,
            params: T,
        ) -> Result<Option<PersistedEntity>> {
            trace!("querying");

            let mut stmt = self.conn.prepare(query)?;

            let mut entities = stmt.query_map(params, |row| {
                Ok(PersistedEntity {
                    key: row.get(0)?,
                    gid: row.get(1)?,
                    version: row.get(2)?,
                    serialized: row.get(3)?,
                })
            })?;

            match entities.next() {
                Some(p) => Ok(Some(p?)),
                _ => Ok(None),
            }
        }

        fn load_by_key(&self, key: &EntityKey) -> Result<Option<PersistedEntity>> {
            self.single_query(
                "SELECT key, gid, version, serialized FROM entities WHERE key = ?;",
                [key.key_to_string()],
            )
        }

        fn load_by_gid(&self, gid: &EntityGid) -> Result<Option<PersistedEntity>> {
            self.single_query(
                "SELECT key, gid, version, serialized FROM entities WHERE gid = ?;",
                [gid.gid_to_string()],
            )
        }
    }

    impl EntityStorage for SqliteStorage {
        fn load(&self, lookup: &LookupBy) -> Result<Option<PersistedEntity>> {
            match lookup {
                LookupBy::Key(key) => self.load_by_key(key),
                LookupBy::Gid(gid) => self.load_by_gid(gid),
            }
        }

        fn save(&self, entity: &PersistedEntity) -> Result<()> {
            let affected = if entity.version == 1 {
                debug!(%entity.key, %entity.gid, "inserting");

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
                debug!(%entity.key, %entity.gid, "updating");

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

        fn delete(&self, entity: &PersistedEntity) -> Result<()> {
            debug!(%entity.key,  %entity.gid, "deleting");

            let mut stmt = self
                .conn
                .prepare("DELETE FROM entities WHERE key = ?1 AND version = ?2")?;

            stmt.execute((&entity.key, &entity.version))?;

            Ok(())
        }

        fn begin(&self) -> Result<()> {
            trace!("tx:begin");

            self.conn.execute("BEGIN TRANSACTION", [])?;

            Ok(())
        }

        fn rollback(&self, benign: bool) -> Result<()> {
            if benign {
                trace!("tx:rollback");
            } else {
                warn!("tx:rollback");
            }

            self.conn.execute("ROLLBACK TRANSACTION", [])?;

            Ok(())
        }

        fn commit(&self) -> Result<()> {
            trace!("tx:commit");

            self.conn.execute("COMMIT TRANSACTION", [])?;

            Ok(())
        }
    }

    pub struct Factory {
        path: String,
        #[allow(dead_code)]
        keep_alive: Option<std::sync::Mutex<Connection>>,
    }

    impl Factory {
        pub fn new(path: &str) -> Result<Arc<Factory>> {
            let connection = Connection::open_with_flags(
                "file:burrow-1?mode=memory&cache=shared",
                OpenFlags::SQLITE_OPEN_URI | OpenFlags::SQLITE_OPEN_READ_WRITE,
            )?;

            Ok(Arc::new(Factory {
                path: path.to_string(),
                keep_alive: Some(std::sync::Mutex::new(connection)),
            }))
        }
    }

    impl EntityStorageFactory for Factory {
        fn create_storage(&self) -> Result<Rc<dyn EntityStorage>> {
            Ok(SqliteStorage::new(&self.path)?)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use anyhow::Result;

        fn get_storage() -> Result<Rc<dyn EntityStorage>> {
            let s = Factory::new(":memory:")?;

            s.create_storage()
        }

        #[test]
        fn it_queries_for_entity_by_missing_key() -> Result<()> {
            let s = get_storage()?;

            assert!(s.load(&LookupBy::Key(&EntityKey::new("world")))?.is_none());

            Ok(())
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

            s.load(&LookupBy::Key(&EntityKey::new("world")))?;

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

            let mut p1 = s.load(&LookupBy::Key(&EntityKey::new("world")))?.unwrap();

            assert_eq!(1, p1.version);

            p1.version += 1;

            s.save(&p1)?;

            let p2 = s.load(&LookupBy::Key(&EntityKey::new("world")))?.unwrap();

            assert_eq!(2, p2.version);

            Ok(())
        }

        #[test]
        fn it_inserts_a_new_entity_in_a_rolled_back_transaction_inserts_nothing() -> Result<()> {
            let s = get_storage()?;

            s.begin()?;

            s.save(&PersistedEntity {
                key: "world".to_string(),
                gid: 1,
                version: 1,
                serialized: "{}".to_string(),
            })?;

            s.rollback(true)?;

            assert!(s.load(&LookupBy::Key(&EntityKey::new("world")))?.is_none());

            Ok(())
        }

        #[test]
        fn it_inserts_a_new_entity_in_a_committed_transaction() -> Result<()> {
            let s = get_storage()?;

            s.begin()?;

            s.save(&PersistedEntity {
                key: "world".to_string(),
                gid: 1,
                version: 1,
                serialized: "{}".to_string(),
            })?;

            s.commit()?;

            s.load(&LookupBy::Key(&EntityKey::new("world")))?;

            Ok(())
        }

        #[test]
        fn it_deletes_entity() -> Result<()> {
            let s = get_storage()?;

            s.save(&PersistedEntity {
                key: "world".to_string(),
                gid: 1,
                version: 1,
                serialized: "{}".to_string(),
            })?;

            let p1 = s.load(&LookupBy::Key(&EntityKey::new("world")))?.unwrap();

            assert_eq!(1, p1.version);

            s.delete(&p1)?;

            let p2 = s.load(&LookupBy::Key(&EntityKey::new("world")))?;

            assert!(p2.is_none());

            Ok(())
        }
    }
}
