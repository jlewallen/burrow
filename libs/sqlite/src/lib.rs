use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use r2d2::PooledConnection;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{Connection, OpenFlags};
use std::{rc::Rc, sync::Mutex};
use tracing::*;

use engine::{
    storage::EntityStorage,
    storage::{FutureStorage, PendingFutures, Storage, StorageFactory},
    storage::{PersistedEntity, PersistedFuture},
};
use kernel::prelude::{EntityGid, EntityKey, LookupBy};

pub const MEMORY_SPECIAL: &str = ":memory:";

pub struct SqliteStorage<C>
where
    C: AsConnection,
{
    conn: C,
}

enum SetupQuery {
    Execute(&'static str),
    Query(&'static str),
}

pub trait AsConnection {
    fn connection(&self) -> &Connection;
}

pub trait Migrate {
    fn migrate(&self) -> Result<()>;
}

impl Migrate for Connection {
    fn migrate(&self) -> Result<()> {
        let exec = |query: SetupQuery| -> Result<()> {
            match query {
                SetupQuery::Execute(sql) => {
                    let mut stmt = self.prepare(sql)?;
                    stmt.execute([])?;
                }
                SetupQuery::Query(sql) => {
                    let mut stmt = self.prepare(sql)?;
                    let _ = stmt.query([])?;
                }
            };
            Ok(())
        };

        exec(SetupQuery::Query("PRAGMA journal_mode = WAL"))?;

        exec(SetupQuery::Execute(
            r#"
                CREATE TABLE IF NOT EXISTS entities (
                    key TEXT NOT NULL PRIMARY KEY,
                    version INTEGER NOT NULL,
                    gid INTEGER,
                    serialized TEXT NOT NULL
                )"#,
        ))?;

        exec(SetupQuery::Execute(
            r#"CREATE UNIQUE INDEX IF NOT EXISTS entities_gid ON entities (gid)"#,
        ))?;

        exec(SetupQuery::Execute(
            r#"
                CREATE TABLE IF NOT EXISTS futures (
                    key TEXT NOT NULL PRIMARY KEY,
                    entity TEXT NOT NULL,
                    time TIMESTAMP NOT NULL,
                    serialized TEXT NOT NULL
                )"#,
        ))?;

        exec(SetupQuery::Execute(
            r#"CREATE UNIQUE INDEX IF NOT EXISTS futures_time ON futures (time)"#,
        ))?;

        Ok(())
    }
}

struct Owned {
    conn: Connection,
}

impl Owned {
    fn new(uri: &str) -> Result<Self> {
        let conn = Connection::open_with_flags(
            uri,
            OpenFlags::SQLITE_OPEN_URI | OpenFlags::SQLITE_OPEN_READ_WRITE,
        )?;

        Ok(Self { conn })
    }
}

impl AsConnection for Owned {
    fn connection(&self) -> &Connection {
        &self.conn
    }
}

struct Pooled {
    conn: PooledConnection<SqliteConnectionManager>,
}

impl AsConnection for Pooled {
    fn connection(&self) -> &Connection {
        &self.conn
    }
}

impl<C> SqliteStorage<C>
where
    C: AsConnection,
{
    pub fn wrap(conn: C) -> Result<Rc<Self>> {
        Ok(Rc::new(Self { conn }))
    }

    fn connection(&self) -> &Connection {
        self.conn.connection()
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

        let mut stmt = self.connection().prepare(query)?;

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

impl<C> FutureStorage for SqliteStorage<C>
where
    C: AsConnection,
{
    fn queue(&self, future: PersistedFuture) -> Result<()> {
        let mut stmt = self.connection().prepare(
            "INSERT OR IGNORE INTO futures (key, entity, time, serialized) VALUES (?1, ?2, ?3, ?4)",
        )?;

        let affected = stmt
            .execute((
                &future.key,
                future.entity.key_to_string(),
                &future.time,
                &future.serialized,
            ))
            .with_context(|| "inserting future")?;

        if affected != 1 {
            warn!(key = %future.key, "schedule:noop");
        } else {
            info!(key = %future.key, time = %future.time, "schedule");
        }

        Ok(())
    }

    fn cancel(&self, key: &str) -> Result<()> {
        let mut stmt = self
            .connection()
            .prepare("DELETE FROM futures WHERE key = ?1")?;

        let affected = stmt.execute((key,)).with_context(|| "cancelling future")?;

        if affected != 1 {
            warn!("cancel:noop");
        } else {
            info!(%key, "cancel");
        }

        Ok(())
    }

    fn query_futures_before(&self, now: DateTime<Utc>) -> Result<PendingFutures> {
        trace!("query-futures {:?}", now);

        let mut stmt = self
            .connection()
            .prepare("SELECT MIN(time) FROM futures WHERE time > ?1 ORDER BY time")?;

        let upcoming: Option<DateTime<Utc>> = stmt.query_row([&now], |row| row.get(0))?;

        trace!(?upcoming, "query-futures");

        let mut stmt = self.connection().prepare(
            "SELECT key, entity, time, serialized FROM futures WHERE time <= ?1 ORDER BY time",
        )?;

        let futures = stmt.query_map([&now], |row| {
            Ok(PersistedFuture {
                key: row.get(0)?,
                entity: EntityKey::from_string(row.get(1)?),
                time: row.get(2)?,
                serialized: row.get(3)?,
            })
        })?;

        let pending: Vec<PersistedFuture> =
            futures.into_iter().map(|v| Ok(v?)).collect::<Result<_>>()?;

        if pending.is_empty() {
            return Ok(PendingFutures::Waiting(upcoming));
        }

        let mut stmt = self
            .connection()
            .prepare("DELETE FROM futures WHERE time <= ?1")
            .with_context(|| "deleting pending futures")?;

        let deleted = stmt.execute((now,))?;

        if deleted != pending.len() {
            warn!(
                pending = %pending.len(),
                deleted = %deleted,
                "query-futures:mismatch",
            );
        }

        Ok(PendingFutures::Futures(pending))
    }
}

impl<C> Storage for SqliteStorage<C> where C: AsConnection {}

impl<C> EntityStorage for SqliteStorage<C>
where
    C: AsConnection,
{
    fn load(&self, lookup: &LookupBy) -> Result<Option<PersistedEntity>> {
        match lookup {
            LookupBy::Key(key) => self.load_by_key(key),
            LookupBy::Gid(gid) => self.load_by_gid(gid),
        }
    }

    fn save(&self, entity: &PersistedEntity) -> Result<()> {
        let affected = if entity.version == 1 {
            debug!(%entity.key, %entity.gid, "inserting");

            let mut stmt = self.connection().prepare(
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

            let mut stmt = self.connection().prepare(
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
            .connection()
            .prepare("DELETE FROM entities WHERE key = ?1 AND version = ?2")?;

        stmt.execute((&entity.key, &entity.version))?;

        Ok(())
    }

    fn begin(&self) -> Result<()> {
        trace!("tx:begin");

        self.connection().execute("BEGIN TRANSACTION", [])?;

        Ok(())
    }

    fn rollback(&self, benign: bool) -> Result<()> {
        if benign {
            trace!("tx:rollback");
        } else {
            warn!("tx:rollback");
        }

        self.connection().execute("ROLLBACK TRANSACTION", [])?;

        Ok(())
    }

    fn commit(&self) -> Result<()> {
        trace!("tx:commit");

        self.connection().execute("COMMIT TRANSACTION", [])?;

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
    pub fn new(path: &str) -> Result<Self> {
        let id = nanoid::nanoid!();
        let (keep_alive, uri) = if path == MEMORY_SPECIAL {
            let keep_alive = InMemoryKeepAlive::new(&id)?;
            let uri = keep_alive.url.to_owned();
            (Some(keep_alive), uri)
        } else {
            (None, format!("file:{}", path))
        };

        Ok(Factory {
            uri,
            _id: id,
            _keep_alive: keep_alive,
        })
    }
}

impl StorageFactory for Factory {
    fn migrate(&self) -> Result<()> {
        let conn = Owned::new(&self.uri)?;
        conn.connection().migrate()
    }

    fn create_storage(&self) -> Result<Rc<dyn Storage>> {
        Ok(SqliteStorage::wrap(Owned::new(&self.uri)?)?)
    }
}

pub struct ConnectionPool {
    pool: r2d2::Pool<SqliteConnectionManager>,
}

impl ConnectionPool {
    pub fn new(path: &str) -> Result<Self> {
        let manager = SqliteConnectionManager::file(path);
        let pool = r2d2::Pool::new(manager)?;
        Ok(Self { pool })
    }
}

impl StorageFactory for ConnectionPool {
    fn migrate(&self) -> Result<()> {
        let conn = self.pool.get()?;
        conn.migrate()
    }

    fn create_storage(&self) -> Result<Rc<dyn Storage>> {
        Ok(SqliteStorage::wrap(Pooled {
            conn: self.pool.get()?,
        })?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use chrono::Days;

    fn get_storage() -> Result<Rc<dyn Storage>> {
        let s = Factory::new(MEMORY_SPECIAL)?;

        s.migrate()?;

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
            entity: EntityKey::new("E-0"),
            time,
            serialized: "{}".to_owned(),
        })?;

        let pending = s.query_futures_before(time.checked_add_days(Days::new(1)).unwrap())?;

        assert_eq!(pending.number_futures(), Some(1));

        let pending = s.query_futures_before(time.checked_add_days(Days::new(1)).unwrap())?;

        assert_eq!(pending, PendingFutures::Waiting(None));

        Ok(())
    }

    #[test]
    fn it_cancels_queued_futures() -> Result<()> {
        let s = get_storage()?;

        let time = Utc::now();

        s.queue(PersistedFuture {
            key: "test-1".to_owned(),
            entity: EntityKey::new("E-0"),
            time,
            serialized: "{}".to_owned(),
        })?;

        s.cancel("test-1")?;

        let pending = s.query_futures_before(time.checked_add_days(Days::new(1)).unwrap())?;

        assert_eq!(pending, PendingFutures::Waiting(None));

        Ok(())
    }
}
