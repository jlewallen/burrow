use std::{
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
};

use crate::*;

use anyhow::Result;
use chrono::{DateTime, Utc};
use prelude::*;

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

impl EntityPtrResolver for KeysOnlySession {
    fn recursive_entity(
        &self,
        _lookup: &LookupBy,
        _depth: usize,
    ) -> Result<Option<EntityPtr>, DomainError> {
        todo!()
    }
}

impl Performer for KeysOnlySession {
    fn perform(&self, _perform: Perform) -> Result<Effect, DomainError> {
        todo!()
    }
}

impl ActiveSession for KeysOnlySession {
    fn new_key(&self) -> EntityKey {
        EntityKey::new(&format!(
            "E-{}",
            self.sequence.fetch_add(1, Ordering::Relaxed)
        ))
    }

    fn new_identity(&self) -> Identity {
        Identity::default()
    }

    fn add_entity(&self, _entity: Entity) -> Result<EntityPtr, DomainError> {
        todo!()
    }

    fn find_item(
        &self,
        _surroundings: &Surroundings,
        _item: &Item,
    ) -> Result<Option<EntityPtr>, DomainError> {
        todo!()
    }

    fn obliterate(&self, _entity: &EntityPtr) -> Result<(), DomainError> {
        todo!()
    }

    fn raise(
        &self,
        _living: Option<EntityPtr>,
        _audience: Audience,
        _raising: Raising,
    ) -> Result<(), DomainError> {
        todo!()
    }

    fn schedule(
        &self,
        _key: String,
        _entity: EntityKey,
        _when: DateTime<Utc>,
        _message: &dyn ToTaggedJson,
    ) -> Result<(), DomainError> {
        todo!()
    }

    fn try_deserialize_action(
        &self,
        _tagged: &TaggedJson,
    ) -> Result<Option<Box<dyn Action>>, serde_json::Error> {
        todo!()
    }

    fn hooks(&self) -> &ManagedHooks {
        todo!()
    }
}
