use anyhow::Result;
use std::{rc::Rc, sync::Arc};
use tracing::info;

use super::{sequences::Sequence, Session};
use crate::{
    sequences::{make_identities, make_keys},
    storage::EntityStorageFactory,
    storage::PersistedEntity,
};
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
        deterministic: bool,
    ) -> Self {
        info!("domain-new");

        Domain {
            storage_factory,
            keys: make_keys(deterministic),
            identities: make_identities(deterministic),
            finder,
            plugins,
        }
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
