use anyhow::Result;
use std::{
    rc::Rc,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};
use tracing::info;

use super::Session;
use crate::{
    kernel::{EntityKey, Identity, RegisteredPlugins},
    plugins::{
        building::BuildingPlugin, carrying::CarryingPlugin, looking::LookingPlugin,
        moving::MovingPlugin,
    },
    storage::EntityStorageFactory,
};

pub trait Sequence<T>: Send + Sync {
    fn following(&self) -> T;
}

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
            keys: if deterministic {
                Arc::new(DeterministicKeys {
                    sequence: AtomicU64::new(0),
                })
            } else {
                Arc::new(RandomKeys {})
            },
            identities: if deterministic {
                Arc::new(DeterministicKeys {
                    sequence: AtomicU64::new(0),
                })
            } else {
                Arc::new(RandomKeys {})
            },
            plugins: Arc::new(plugins),
        }
    }

    pub fn open_session(&self) -> Result<Rc<Session>> {
        info!("session-open");

        let storage = self.storage_factory.create_storage()?;

        Session::new(storage, &self.keys, &self.identities, &self.plugins)
    }
}

struct DeterministicKeys {
    sequence: AtomicU64,
}

impl Sequence<EntityKey> for DeterministicKeys {
    fn following(&self) -> EntityKey {
        EntityKey::new(&format!(
            "E-{}",
            self.sequence.fetch_add(1, Ordering::Relaxed)
        ))
    }
}

impl Sequence<Identity> for DeterministicKeys {
    fn following(&self) -> Identity {
        let unique = self.sequence.fetch_add(1, Ordering::Relaxed);
        let public = format!("Public#{}", unique);
        let private = format!("Private#{}", unique);
        Identity::new(public, private)
    }
}

struct RandomKeys {}

impl Sequence<EntityKey> for RandomKeys {
    fn following(&self) -> EntityKey {
        EntityKey::default()
    }
}

impl Sequence<Identity> for RandomKeys {
    fn following(&self) -> Identity {
        Identity::default()
    }
}
