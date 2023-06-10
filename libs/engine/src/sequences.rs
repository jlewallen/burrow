use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use kernel::{EntityGid, EntityKey, Identity};

pub trait Sequence<T>: Send + Sync {
    fn following(&self) -> T;
}

pub struct DeterministicKeys {
    sequence: AtomicU64,
}

impl DeterministicKeys {
    pub fn new() -> Self {
        Self {
            sequence: AtomicU64::new(0),
        }
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

#[derive(Debug)]
pub struct GlobalIds {
    gid: AtomicU64,
}

impl GlobalIds {
    pub fn new() -> Rc<Self> {
        Rc::new(Self {
            gid: AtomicU64::new(0),
        })
    }

    pub fn gid(&self) -> EntityGid {
        EntityGid::new(self.gid.load(Ordering::Relaxed))
    }

    pub fn set(&self, gid: &EntityGid) {
        self.gid.store(gid.into(), Ordering::Relaxed);
    }

    pub fn get(&self) -> EntityGid {
        EntityGid::new(self.gid.fetch_add(1, Ordering::Relaxed) + 1)
    }
}
