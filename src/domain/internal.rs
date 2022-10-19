use crate::kernel::*;
use crate::storage::EntityStorage;
use anyhow::Result;
use std::{
    cell::RefCell,
    fmt::Debug,
    rc::{Rc, Weak},
};
use tracing::{debug, info, span, trace, Level};

struct Entities {
    entities: RefCell<Vec<Rc<RefCell<Entity>>>>,
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
            entities: RefCell::new(Vec::new()),
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
    ) -> Result<Rc<RefCell<Entity>>, DomainError> {
        {
            let check_existing = self.entities.borrow();
            for row in check_existing.iter() {
                if row.borrow().key == *key {
                    debug!(%key, "existing");
                    return Ok(Rc::clone(row));
                }
            }
        }

        let _loading_span = span!(Level::INFO, "entity", key = key.key_to_string()).entered();

        info!("loading");
        let persisted = self.storage.load(key)?;

        trace!("parsing");
        let mut loaded: Entity = serde_json::from_str(&persisted.serialized)?;

        trace!("infrastructure");
        loaded.prepare_with(&self.infra)?;

        prepare(&mut loaded)?;

        let cell = Rc::new(RefCell::new(loaded));

        let mut add_new = self.entities.borrow_mut();

        add_new.push(Rc::clone(&cell));

        Ok(cell)
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
    fn load_entity_by_key(&self, key: &EntityKey) -> Result<Rc<RefCell<Entity>>, DomainError> {
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
            } => Ok(DynamicEntityRef::Entity(ReferencedEntity::new(
                self.load_entity_by_key(key)?,
            ))),
            DynamicEntityRef::Entity(_) => Ok(entity_ref.clone()),
        }
    }
}
