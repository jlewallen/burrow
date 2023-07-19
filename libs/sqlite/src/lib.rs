use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use std::{
    rc::Rc,
    sync::{Arc, Mutex},
};

use engine::{
    storage::EntityStorage,
    storage::{EntityStorageFactory, FutureStorage},
    storage::{PersistedEntity, PersistedFuture},
};
use kernel::{EntityGid, EntityKey, LookupBy};
use rusqlite::{Connection, OpenFlags};
use tracing::*;

pub const MEMORY_SPECIAL: &str = ":memory:";

pub struct SqliteStorage {
    conn: Connection,
}

impl SqliteStorage {
    pub fn new(uri: &str) -> Result<Rc<Self>> {
        let conn = Connection::open_with_flags(
            uri,
            OpenFlags::SQLITE_OPEN_URI | OpenFlags::SQLITE_OPEN_READ_WRITE,
        )?;

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

        exec(
            r#"
                CREATE TABLE IF NOT EXISTS futures (
                    key TEXT NOT NULL PRIMARY KEY,
                    time TIMESTAMP NOT NULL,
                    serialized TEXT NOT NULL
                )"#,
        )?;

        exec(r#"CREATE UNIQUE INDEX IF NOT EXISTS futures_time ON futures (time)"#)?;

        Ok(Rc::new(SqliteStorage { conn }))
    }

    fn single_query<T: rusqlite::Params>(
        &self,
        query: &str,
        params: T,
    ) -> Result<Option<PersistedEntity>> {
        let mut unknown = self.multiple_query(query, params)?;
        match unknown.len() {
            0 => Ok(None),
            1 => Ok(Some(unknown.remove(0))),
            _ => Err(anyhow!("Unexpected number of rows for single query")),
        }
    }

    fn multiple_query<T: rusqlite::Params>(
        &self,
        query: &str,
        params: T,
    ) -> Result<Vec<PersistedEntity>> {
        trace!("querying");

        let mut stmt = self.conn.prepare(query)?;

        let entities = stmt.query_map(params, |row| {
            Ok(PersistedEntity {
                key: row.get(0)?,
                gid: row.get(1)?,
                version: row.get(2)?,
                serialized: row.get(3)?,
            })
        })?;

        entities.into_iter().map(|v| Ok(v?)).collect::<Result<_>>()
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

impl FutureStorage for SqliteStorage {
    fn queue(&self, future: PersistedFuture) -> Result<()> {
        let mut stmt = self
            .conn
            .prepare("INSERT OR IGNORE INTO futures (key, time, serialized) VALUES (?1, ?2, ?3)")?;

        let affected = stmt.execute((&future.key, &future.time, &future.serialized))?;

        if affected != 1 {
            warn!(key = %future.key, "schedule:noop");
        } else {
            info!(key = %future.key, time = %future.time, "schedule");
        }

        Ok(())
    }

    fn cancel(&self, key: &str) -> Result<()> {
        let mut stmt = self.conn.prepare("DELETE FROM futures WHERE key = ?1")?;

        let affected = stmt.execute((key,))?;

        if affected != 1 {
            warn!("cancel:noop");
        } else {
            info!(%key, "cancel");
        }

        Ok(())
    }

    fn query_futures_before(&self, now: DateTime<Utc>) -> Result<Vec<PersistedFuture>> {
        trace!("query-futures {:?}", now);

        let mut stmt = self
            .conn
            .prepare("SELECT key, time, serialized FROM futures WHERE time <= ?1 ORDER BY time")?;

        let futures = stmt.query_map((&now,), |row| {
            Ok(PersistedFuture {
                key: row.get(0)?,
                time: row.get(1)?,
                serialized: row.get(2)?,
            })
        })?;

        let pending: Vec<PersistedFuture> =
            futures.into_iter().map(|v| Ok(v?)).collect::<Result<_>>()?;

        let mut stmt = self.conn.prepare("DELETE FROM futures WHERE time <= ?1")?;

        let deleted = stmt.execute((now,))?;

        if deleted != pending.len() {
            warn!(
                pending = %pending.len(),
                deleted = %deleted,
                "query-futures:mismatch",
            );
        }

        Ok(pending)
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

    fn query_all(&self) -> Result<Vec<PersistedEntity>> {
        self.multiple_query(
            "SELECT key, gid, version, serialized FROM entities ORDER BY gid;",
            [],
        )
    }
}

struct InMemoryKeepAlive {
    _connection: Mutex<Connection>,
    url: String,
}

impl InMemoryKeepAlive {
    fn new(id: &str) -> Result<Self> {
        let url = format!("file:burrow-{}?mode=memory&cache=shared", id);
        Ok(Self {
            _connection: Mutex::new(Connection::open_with_flags(
                &url,
                OpenFlags::SQLITE_OPEN_URI | OpenFlags::SQLITE_OPEN_READ_WRITE,
            )?),
            url,
        })
    }
}

pub struct Factory {
    uri: String,
    _id: String,
    _keep_alive: Option<InMemoryKeepAlive>,
}

impl Factory {
    pub fn new(path: &str) -> Result<Arc<Factory>> {
        let id = nanoid::nanoid!();
        let (keep_alive, uri) = if path == MEMORY_SPECIAL {
            let keep_alive = InMemoryKeepAlive::new(&id)?;
            let uri = keep_alive.url.to_owned();
            (Some(keep_alive), uri)
        } else {
            (None, format!("file:{}", path))
        };

        Ok(Arc::new(Factory {
            uri,
            _id: id,
            _keep_alive: keep_alive,
        }))
    }
}

impl EntityStorageFactory for Factory {
    fn create_storage(&self) -> Result<Rc<dyn EntityStorage>> {
        Ok(SqliteStorage::new(&self.uri)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use chrono::Days;

    fn get_storage() -> Result<Rc<dyn EntityStorage>> {
        let s = Factory::new(MEMORY_SPECIAL)?;

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

    #[test]
    fn it_queues_futures() -> Result<()> {
        let s = get_storage()?;

        let time = Utc::now();

        s.queue(PersistedFuture {
            key: "test-1".to_owned(),
            time,
            serialized: "{}".to_owned(),
        })?;

        let pending = s.query_futures_before(time.checked_add_days(Days::new(1)).unwrap())?;

        assert_eq!(pending.len(), 1);

        let pending = s.query_futures_before(time.checked_add_days(Days::new(1)).unwrap())?;

        assert_eq!(pending.len(), 0);

        Ok(())
    }

    #[test]
    fn it_cancels_queued_futures() -> Result<()> {
        let s = get_storage()?;

        let time = Utc::now();

        s.queue(PersistedFuture {
            key: "test-1".to_owned(),
            time,
            serialized: "{}".to_owned(),
        })?;

        s.cancel("test-1")?;

        let pending = s.query_futures_before(time.checked_add_days(Days::new(1)).unwrap())?;

        assert_eq!(pending.len(), 0);

        Ok(())
    }
}
