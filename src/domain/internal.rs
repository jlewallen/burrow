use crate::kernel::*;
use crate::storage::EntityStorage;
use anyhow::Result;
use elsa::FrozenMap;
use std::{
    fmt::Debug,
    rc::{Rc, Weak},
};
use tracing::{debug, info, span, trace, Level};

struct Entities {
    entities: FrozenMap<EntityKey, Box<Entity>>,
    storage: Box<dyn EntityStorage>,
    infra: Weak<dyn Infrastructure>,
}

impl Debug for Entities {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Entities").finish()
    }
}

impl Entities {
    pub fn new(storage: Box<dyn EntityStorage>, infra: Weak<dyn Infrastructure>) -> Rc<Self> {
        trace!("entities-new");

        Rc::new(Self {
            entities: FrozenMap::new(),
            storage,
            infra,
        })
    }
}

impl PrepareEntities for Entities {
    fn prepare_entity_by_key<T: Fn(&mut Entity) -> Result<()>>(
        &self,
        key: &EntityKey,
        prepare: T,
    ) -> Result<&Entity, DomainError> {
        if let Some(e) = self.entities.get(key) {
            debug!(%key, "existing");
            return Ok(e);
        }

        let _loading_span = span!(Level::INFO, "entity", key = key.key_to_string()).entered();

        info!("loading");
        let persisted = self.storage.load(key)?;

        trace!("parsing");
        let mut loaded: Entity = serde_json::from_str(&persisted.serialized)?;

        trace!("infrastructure");
        loaded.prepare_with(&self.infra)?;

        prepare(&mut loaded)?;

        let inserted = self.entities.insert(key.clone(), Box::new(loaded));

        Ok(inserted)
    }
}

#[derive(Debug)]
pub struct DomainInfrastructure {
    entities: Rc<Entities>,
}

impl DomainInfrastructure {
    pub fn new(storage: Box<dyn EntityStorage>) -> Rc<Self> {
        Rc::new_cyclic(|me: &Weak<DomainInfrastructure>| {
            // How acceptable is this kind of thing?
            let infra = Weak::clone(me) as Weak<dyn Infrastructure>;
            let entities = Entities::new(storage, infra);
            DomainInfrastructure { entities }
        })
    }
}

impl LoadEntities for DomainInfrastructure {
    fn load_entity_by_key(&self, key: &EntityKey) -> Result<&Entity, DomainError> {
        self.entities.prepare_entity_by_key(key, |_e| Ok(()))
    }
}

impl Infrastructure for DomainInfrastructure {
    fn ensure_entity(&self, entity_ref: &DynamicEntityRef) -> Result<DynamicEntityRef> {
        match entity_ref {
            DynamicEntityRef::RefOnly {
                py_object: _,
                py_ref: _,
                key,
                class: _,
                name: _,
            } => Ok(DynamicEntityRef::Entity(Box::new(
                // TODO This clone will become a problem.
                self.load_entity_by_key(key)?.clone(),
            ))),
            DynamicEntityRef::Entity(_) => Ok(entity_ref.clone()),
        }
    }
}
