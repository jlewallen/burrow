use anyhow::Result;
use chrono::{DateTime, Utc};
use std::{rc::Rc, sync::Arc};
use tracing::{info, trace};

use crate::{
    notifications::Notifier,
    prelude::{Dependencies, USER_DEPTH},
    sequences::Sequence,
    session::Session,
    storage::{PendingFutures, PersistedEntity, StorageFactory},
};
use kernel::prelude::*;

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

                use itertools::Itertools;
                let futures_by_actor = futures.into_iter().group_by(|f| f.entity.clone());

                for (_key, futures) in futures_by_actor.into_iter() {
                    let session = session.set_session()?;

                    for future in futures {
                        info!(key = %future.key, entity = %future.entity, time = %future.time, "delivering");

                        let value = serde_json::from_str(&future.serialized)?;
                        if let Ok(Some(action)) = session.try_deserialize_action(&value) {
                            if let Some(actor) = session
                                .recursive_entity(&LookupBy::Key(&future.entity), USER_DEPTH)?
                            {
                                session.captured(actor, action)?;
                            }
                        }
                    }

                    session.close(notifier)?;
                }

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

        Session::new(
            Dependencies::new(
                &self.keys,
                &self.identities,
                &self.finder,
                &self.plugins,
                &self.storage_factory,
            ),
            middleware,
        )
    }
}

impl SessionOpener for Domain {
    fn open_session(&self) -> Result<Rc<Session>> {
        self.open_session_with_middleware(vec![])
    }
}
