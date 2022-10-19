use crate::storage::EntityStorage;
use crate::{kernel::*, plugins::carrying::model::Containing};
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
    ) -> Result<EntityPtr> {
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
    fn load_entity_by_key(&self, key: &EntityKey) -> Result<EntityPtr> {
        self.entities.prepare_entity_by_key(key, |_e| Ok(()))
    }
}

#[derive(Debug, Clone)]
enum EntityRelationship {
    World(EntityPtr),
    User(EntityPtr),
    Area(EntityPtr),
    Holding(EntityPtr),
    Ground(EntityPtr),
}

pub struct EntityRelationshipSet {
    entities: Vec<EntityRelationship>,
}

impl EntityRelationshipSet {
    fn new_from_action((world, user, area, _infra): ActionArgs) -> Self {
        Self {
            entities: vec![
                EntityRelationship::World(world),
                EntityRelationship::User(user),
                EntityRelationship::Area(area),
            ],
        }
    }

    fn expand(&self) -> Result<Self> {
        let mut expanded = self.entities.clone();

        for entity in &self.entities {
            match entity {
                EntityRelationship::World(_world) => {}
                EntityRelationship::User(user) => {
                    if let Ok(containing) = user.borrow().scope::<Containing>() {
                        for entity in containing.holding {
                            expanded.push(EntityRelationship::Holding(entity.try_into()?));
                        }
                    }
                }
                EntityRelationship::Area(area) => {
                    if let Ok(containing) = area.borrow().scope::<Containing>() {
                        for entity in containing.holding {
                            expanded.push(EntityRelationship::Ground(entity.try_into()?));
                        }
                    }
                }
                EntityRelationship::Holding(_holding) => {}
                EntityRelationship::Ground(_ground) => {}
            }
        }

        Ok(Self { entities: expanded })
    }
}

/// Determines if an entity matches a user's description of that entity, given
/// no other context at all.
fn matches_description(entity: &Entity, desc: &str) -> bool {
    if let Some(name) = entity.name() {
        name.contains(desc)
    } else {
        false
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

    fn find_item(&self, args: ActionArgs, item: &Item) -> Result<Option<EntityPtr>> {
        match item {
            Item::Named(name) => {
                let haystack = EntityRelationshipSet::new_from_action(args).expand()?;

                for entity in &haystack.entities {
                    match entity {
                        EntityRelationship::Holding(e) => {
                            if matches_description(&e.borrow(), name) {
                                return Ok(Some(e.clone()));
                            }
                        }
                        EntityRelationship::Ground(e) => {
                            if matches_description(&e.borrow(), name) {
                                return Ok(Some(e.clone()));
                            }
                        }
                        _ => {}
                    }
                }

                Ok(None)
            }
        }
    }
}
