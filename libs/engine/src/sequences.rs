use std::sync::atomic::{AtomicU64, Ordering};

use kernel::prelude::{EntityKey, Identity};

pub trait Sequence<T>: Send + Sync {
    fn following(&self) -> T;
}

#[derive(Default)]
pub struct DeterministicKeys {
    sequence: AtomicU64,
}

impl DeterministicKeys {
    pub fn new() -> Self {
        Self::default()
    }
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
