use super::Session;
use crate::{
    kernel::{EntityKey, Identity},
    storage::EntityStorageFactory,
};
use anyhow::Result;
use std::{
    rc::Rc,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};
use tracing::info;

pub trait Sequence<T>: Send + Sync {
    fn following(&self) -> T;
}

pub struct Domain {
    storage_factory: Box<dyn EntityStorageFactory>,
    keys: Arc<dyn Sequence<EntityKey>>,
    identities: Arc<dyn Sequence<Identity>>,
}

impl Domain {
    pub fn new(storage_factory: Box<dyn EntityStorageFactory>, deterministic: bool) -> Self {
        info!("domain-new");

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
        }
    }

    pub fn open_session(&self) -> Result<Rc<Session>> {
        info!("session-open");

        let storage = self.storage_factory.create_storage()?;

        Session::new(storage, &self.keys, &self.identities)
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
