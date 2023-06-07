use nanoid::nanoid;
use std::rc::Rc;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

use kernel::{EntityGid, EntityKey, Identity};

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
        EntityKey::from_string(nanoid!())
    }
}

impl Sequence<Identity> for RandomKeys {
    fn following(&self) -> Identity {
        Identity::default()
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
