use crate::storage::EntityStorage;
use crate::{kernel::*, plugins::carrying::model::Containing};
use anyhow::{anyhow, Result};
use std::{
    cell::RefCell,
    collections::HashMap,
    fmt::Debug,
    rc::{Rc, Weak},
};
use tracing::{debug, info, span, trace, Level};

struct Entities {
    entities: RefCell<HashMap<EntityKey, Rc<RefCell<Entity>>>>,
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
            entities: RefCell::new(HashMap::new()),
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
            if let Some(e) = check_existing.get(key) {
                debug!(%key, "existing");
                return Ok(Rc::clone(e));
            }
        }

        let _loading_span = span!(Level::INFO, "entity", key = key.key_to_string()).entered();

        info!("loading");
        let persisted = self.storage.load(key)?;

        trace!("parsing");
        let mut loaded: Entity = serde_json::from_str(&persisted.serialized)?;

        trace!("infrastructure");
        if let Some(infra) = self.infra.upgrade() {
            loaded.supply(&infra)?;
        } else {
            return Err(anyhow!("no infrastructure"));
        }

        prepare(&mut loaded)?;

        let cell = Rc::new(RefCell::new(loaded));

        let mut cache = self.entities.borrow_mut();

        cache.insert(key.clone(), Rc::clone(&cell));

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

        // https://github.com/ferrous-systems/elements-of-rust#tuple-structs-and-enum-tuple-variants-as-functions
        for entity in &self.entities {
            match entity {
                EntityRelationship::World(_world) => {}
                EntityRelationship::User(user) => {
                    let user = user.borrow();
                    if let Ok(containing) = user.scope::<Containing>() {
                        for entity in &containing.holding {
                            expanded.push(EntityRelationship::Holding(entity.into_entity()?));
                        }
                    }
                }
                EntityRelationship::Area(area) => {
                    let area = area.borrow();
                    if let Ok(containing) = area.scope::<Containing>() {
                        for entity in &containing.holding {
                            expanded.push(EntityRelationship::Ground(entity.into_entity()?));
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
    fn ensure_entity(&self, entity_ref: &LazyLoadedEntity) -> Result<LazyLoadedEntity> {
        if entity_ref.has_entity() {
            Ok(entity_ref.clone())
        } else {
            let entity = self.load_entity_by_key(&entity_ref.key)?;
            Ok(LazyLoadedEntity::new_with_entity(entity))
        }
    }

    fn find_item(&self, args: ActionArgs, item: &Item) -> Result<Option<EntityPtr>> {
        let _loading_span = span!(Level::INFO, "finding", i = format!("{:?}", item)).entered();

        info!("finding");

        match item {
            Item::Named(name) => {
                let haystack = EntityRelationshipSet::new_from_action(args).expand()?;

                // https://github.com/ferrous-systems/elements-of-rust#tuple-structs-and-enum-tuple-variants-as-functions
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
