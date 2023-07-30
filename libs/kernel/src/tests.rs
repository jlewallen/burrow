use std::sync::atomic::{AtomicU64, Ordering};

use crate::*;

use anyhow::Result;

#[test]
fn it_creates_expected_json_for_new_named() -> Result<()> {
    let _session = KeysOnlySession::open();
    let e: Entity = build_entity()
        .name("New Named")
        .desc("Description of New Named")
        .try_into()?;

    insta::assert_json_snapshot!(e.to_json_value()?);

    Ok(())
}

#[derive(Default)]
struct KeysOnlySession {
    sequence: AtomicU64,
}

impl KeysOnlySession {
    pub fn new() -> Rc<dyn ActiveSession> {
        Rc::new(Self::default())
    }

    pub fn open() -> SetSession {
        SetSession::new(&KeysOnlySession::new())
    }
}

impl EntryResolver for KeysOnlySession {
    fn entry(&self, _lookup: &LookupBy) -> Result<Option<Entry>, DomainError> {
        todo!()
    }
}

impl Performer for KeysOnlySession {
    fn perform(&self, _perform: Perform) -> Result<Effect> {
        todo!()
    }
}

impl ActiveSession for KeysOnlySession {
    fn find_item(&self, _surroundings: &Surroundings, _item: &Item) -> Result<Option<Entry>> {
        todo!()
    }

    fn ensure_entity(&self, _entity_ref: &EntityRef) -> Result<EntityRef, DomainError> {
        todo!()
    }

    fn add_entity(&self, _entity: &EntityPtr) -> Result<Entry> {
        todo!()
    }

    fn obliterate(&self, _entity: &Entry) -> Result<()> {
        todo!()
    }

    fn new_key(&self) -> EntityKey {
        EntityKey::new(&format!(
            "E-{}",
            self.sequence.fetch_add(1, Ordering::Relaxed)
        ))
    }

    fn new_identity(&self) -> Identity {
        todo!()
    }

    fn raise(&self, _audience: Audience, _event: Box<dyn DomainEvent>) -> Result<()> {
        todo!()
    }

    fn hooks(&self) -> &ManagedHooks {
        todo!()
    }

    fn schedule(&self, _key: &str, _when: When, _message: &dyn ToJson) -> Result<()> {
        todo!()
    }
}
