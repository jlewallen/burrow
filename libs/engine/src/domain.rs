use anyhow::Result;
use chrono::{DateTime, Utc};
use std::{rc::Rc, sync::Arc};
use tracing::{info, trace};

use super::{sequences::Sequence, Session};
use crate::{
    storage::PersistedEntity,
    storage::{PendingFutures, StorageFactory},
    Notifier,
};
use kernel::{EntityKey, Finder, Identity, Incoming, LookupBy, Middleware, RegisteredPlugins};

pub trait SessionOpener: Send + Sync + Clone {
    fn open_session(&self) -> Result<Rc<Session>>;
}

pub enum AfterTick {
    Deadline(DateTime<Utc>),
    Processed(usize),
    Empty,
}

#[derive(Clone)]
pub struct Domain {
    storage_factory: Arc<dyn StorageFactory>,
    keys: Arc<dyn Sequence<EntityKey>>,
    identities: Arc<dyn Sequence<Identity>>,
    finder: Arc<dyn Finder>,
    plugins: Arc<RegisteredPlugins>,
}

impl Domain {
    pub fn new(
        storage_factory: Arc<dyn StorageFactory>,
        plugins: Arc<RegisteredPlugins>,
        finder: Arc<dyn Finder>,
        keys: Arc<dyn Sequence<EntityKey>>,
        identities: Arc<dyn Sequence<Identity>>,
    ) -> Self {
        info!("domain-new");

        Domain {
            storage_factory,
            keys,
            identities,
            finder,
            plugins,
        }
    }

    pub fn tick<T: Notifier>(&self, now: DateTime<Utc>, notifier: &T) -> Result<AfterTick> {
        trace!("{:?} tick", now);

        let storage = self.storage_factory.create_storage()?;
        match storage.query_futures_before(now)? {
            PendingFutures::Futures(futures) => {
                let session = self.open_session()?;
                let processing = futures.len();

                for future in futures {
                    info!(key = %future.key, time = %future.time, "delivering");

                    // TODO We should build a list of known prefixes so we don't need to
                    // iterate over all plugins.
                    session.deliver(Incoming::new(
                        future.key,
                        serde_json::from_str(&future.serialized)?,
                    ))?;
                }

                session.close(notifier)?;

                Ok(AfterTick::Processed(processing))
            }
            PendingFutures::Waiting(Some(deadline)) => Ok(AfterTick::Deadline(deadline)),
            PendingFutures::Waiting(None) => Ok(AfterTick::Empty),
        }
    }

    pub fn query_all(&self) -> Result<Vec<PersistedEntity>> {
        let storage = self.storage_factory.create_storage()?;
        storage.query_all()
    }

    pub fn query_entity(&self, lookup: &LookupBy) -> Result<Option<PersistedEntity>> {
        let storage = self.storage_factory.create_storage()?;
        storage.load(lookup)
    }

    pub fn stop(&self) -> Result<()> {
        self.plugins.stop()?;

        Ok(())
    }

    pub fn open_session_with_middleware(
        &self,
        middleware: Vec<Rc<dyn Middleware>>,
    ) -> Result<Rc<Session>> {
        info!("session-open");

        let storage = self.storage_factory.create_storage()?;

        Session::new(
            &self.keys,
            &self.identities,
            &self.finder,
            &self.plugins,
            storage,
            middleware,
        )
    }
}

impl SessionOpener for Domain {
    fn open_session(&self) -> Result<Rc<Session>> {
        self.open_session_with_middleware(vec![])
    }
}
