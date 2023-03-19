use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

use crate::kernel::{EntityKey, Identity};

pub fn make_keys(deterministic: bool) -> Arc<dyn Sequence<EntityKey>> {
    if deterministic {
        Arc::new(DeterministicKeys {
            sequence: AtomicU64::new(0),
        })
    } else {
        Arc::new(RandomKeys {})
    }
}

pub fn make_identities(deterministic: bool) -> Arc<dyn Sequence<Identity>> {
    if deterministic {
        Arc::new(DeterministicKeys {
            sequence: AtomicU64::new(0),
        })
    } else {
        Arc::new(RandomKeys {})
    }
}

pub trait Sequence<T>: Send + Sync {
    fn following(&self) -> T;
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
