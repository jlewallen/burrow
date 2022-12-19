use super::{IdentityFactory, KeySequence, Session};
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

pub struct Domain {
    storage_factory: Box<dyn EntityStorageFactory>,
    keys: Arc<dyn KeySequence>,
    identities: Arc<dyn IdentityFactory>,
}

impl Domain {
    pub fn new(storage_factory: Box<dyn EntityStorageFactory>, deterministic_keys: bool) -> Self {
        info!("domain-new");

        Domain {
            storage_factory,
            keys: if deterministic_keys {
                Arc::new(DeterministicKeys {
                    sequence: AtomicU64::new(0),
                })
            } else {
                Arc::new(RandomKeys {})
            },
            identities: if deterministic_keys {
                Arc::new(DeterministicIdentities {
                    sequence: AtomicU64::new(0),
                })
            } else {
                Arc::new(RandomIdentities {})
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

impl KeySequence for DeterministicKeys {
    fn new_key(&self) -> EntityKey {
        EntityKey::new(&format!(
            "E-{}",
            self.sequence.fetch_add(1, Ordering::Relaxed)
        ))
    }
}

struct RandomKeys {}

impl KeySequence for RandomKeys {
    fn new_key(&self) -> EntityKey {
        EntityKey::default()
    }
}

struct DeterministicIdentities {
    sequence: AtomicU64,
}

impl IdentityFactory for DeterministicIdentities {
    fn new_identity(&self) -> Identity {
        let unique = self.sequence.fetch_add(1, Ordering::Relaxed);
        let public = format!("Public#{}", unique);
        let private = format!("Private#{}", unique);
        Identity::new(public, private)
    }
}

struct RandomIdentities {}

impl IdentityFactory for RandomIdentities {
    fn new_identity(&self) -> Identity {
        Identity::default()
    }
}
