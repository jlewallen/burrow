use anyhow::Result;
use std::{rc::Rc, sync::Arc};
use tracing::info;

use super::{sequences::Sequence, Session};
use crate::{
    domain::sequences::{make_identities, make_keys},
    kernel::{EntityKey, Identity, RegisteredPlugins},
    plugins::{
        building::BuildingPlugin, carrying::CarryingPlugin, looking::LookingPlugin,
        moving::MovingPlugin,
    },
    storage::EntityStorageFactory,
};

pub struct Domain {
    storage_factory: Box<dyn EntityStorageFactory>,
    keys: Arc<dyn Sequence<EntityKey>>,
    identities: Arc<dyn Sequence<Identity>>,
    plugins: Arc<RegisteredPlugins>,
}

impl Domain {
    pub fn new(storage_factory: Box<dyn EntityStorageFactory>, deterministic: bool) -> Self {
        info!("domain-new");

        let mut plugins: RegisteredPlugins = Default::default();
        plugins.register::<MovingPlugin>();
        plugins.register::<LookingPlugin>();
        plugins.register::<CarryingPlugin>();
        plugins.register::<BuildingPlugin>();

        Domain {
            storage_factory,
            keys: make_keys(deterministic),
            identities: make_identities(deterministic),
            plugins: Arc::new(plugins),
        }
    }

    pub fn open_session(&self) -> Result<Rc<Session>> {
        info!("session-open");

        let storage = self.storage_factory.create_storage()?;

        Session::new(storage, &self.keys, &self.identities, &self.plugins)
    }
}
