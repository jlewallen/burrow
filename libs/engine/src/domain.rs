use anyhow::Result;
use std::{rc::Rc, sync::Arc};
use tracing::info;

use super::{sequences::Sequence, Session};
use crate::{
    sequences::{make_identities, make_keys},
    storage::EntityStorageFactory,
    Finder,
};
use kernel::{EntityKey, Identity, RegisteredPlugins};

pub struct Domain {
    storage_factory: Box<dyn EntityStorageFactory>,
    keys: Arc<dyn Sequence<EntityKey>>,
    identities: Arc<dyn Sequence<Identity>>,
    plugins: Arc<RegisteredPlugins>,
    finder: Arc<dyn Finder>,
}

impl Domain {
    pub fn new(
        storage_factory: Box<dyn EntityStorageFactory>,
        plugins: Arc<RegisteredPlugins>,
        finder: Arc<dyn Finder>,
        deterministic: bool,
    ) -> Self {
        info!("domain-new");

        Domain {
            storage_factory,
            keys: make_keys(deterministic),
            identities: make_identities(deterministic),
            plugins,
            finder,
        }
    }

    pub fn open_session(&self) -> Result<Rc<Session>> {
        info!("session-open");

        let storage = self.storage_factory.create_storage()?;

        Session::new(
            storage,
            &self.keys,
            &self.identities,
            &self.plugins,
            &self.finder,
        )
    }
}
