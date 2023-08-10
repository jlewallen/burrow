use std::sync::atomic::{AtomicU64, Ordering};

use crate::*;

use anyhow::Result;

#[test]
fn it_creates_expected_json_for_new_named() -> Result<()> {
    let _session = KeysOnlySession::open();
    let jacob: Entity = build_entity()
        .class(EntityClass::world())
        .name("Jacob")
        .desc("Hah")
        .try_into()?;

    let world: Entity = build_entity()
        .class(EntityClass::world())
        .creator(jacob.entity_ref())
        .name("New World")
        .desc("What a great place")
        .try_into()?;

    insta::assert_json_snapshot!([world.to_json_value()?, jacob.to_json_value()?]);

    Ok(())
}

#[derive(Default)]
struct KeysOnlySession {
    sequence: AtomicU64,
}

impl KeysOnlySession {
    pub fn open() -> SetSession<KeysOnlySession> {
        SetSession::new(Rc::new(Self::default()))
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
        Identity::default()
    }

    fn raise(&self, _audience: Audience, _raising: Raising) -> Result<()> {
        todo!()
    }

    fn hooks(&self) -> &ManagedHooks {
        todo!()
    }

    fn schedule(&self, _key: &str, _when: When, _message: &dyn ToJson) -> Result<()> {
        todo!()
    }
}
