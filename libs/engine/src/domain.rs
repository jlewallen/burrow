use anyhow::Result;
use chrono::{DateTime, Utc};
use std::{rc::Rc, sync::Arc};
use tracing::{info, trace};

use super::{sequences::Sequence, Session};
use crate::{storage::EntityStorageFactory, storage::PersistedEntity};
use kernel::{EntityKey, Finder, Identity, RegisteredPlugins};

pub trait SessionOpener: Send + Sync + Clone {
    fn open_session(&self) -> Result<Rc<Session>>;
}

#[derive(Clone)]
pub struct Domain {
    storage_factory: Arc<dyn EntityStorageFactory>,
    keys: Arc<dyn Sequence<EntityKey>>,
    identities: Arc<dyn Sequence<Identity>>,
    finder: Arc<dyn Finder>,
    plugins: Arc<RegisteredPlugins>,
}

impl Domain {
    pub fn new(
        storage_factory: Arc<dyn EntityStorageFactory>,
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

    pub fn tick(&self, now: DateTime<Utc>) -> Result<()> {
        trace!("{:?} tick", now);

        let storage = self.storage_factory.create_storage()?;
        let futures = storage.query_futures_before(now)?;
        for future in futures {
            info!(key = %future.key, time = %future.time, "delivering");
        }

        Ok(())
    }

    pub fn query_all(&self) -> Result<Vec<PersistedEntity>> {
        let storage = self.storage_factory.create_storage()?;
        storage.query_all()
    }

    pub fn stop(&self) -> Result<()> {
        self.plugins.stop()?;

        Ok(())
    }
}

impl SessionOpener for Domain {
    fn open_session(&self) -> Result<Rc<Session>> {
        info!("session-open");

        let storage = self.storage_factory.create_storage()?;

        Session::new(
            storage,
            &self.keys,
            &self.identities,
            &self.finder,
            &self.plugins,
        )
    }
}
