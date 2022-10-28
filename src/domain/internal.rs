use anyhow::{anyhow, Result};
use std::{
    cell::RefCell,
    collections::HashMap,
    fmt::Debug,
    rc::{Rc, Weak},
};
use tracing::{debug, info, span, trace, Level};

use crate::storage::EntityStorage;
use crate::{kernel::*, plugins::carrying::model::Containing};

/// Determines if an entity matches a user's description of that entity, given
/// no other context at all.
fn matches_description(entity: &Entity, desc: &str) -> bool {
    if let Some(name) = entity.name() {
        // TODO We can do this more efficiently.
        name.to_lowercase().contains(&desc.to_lowercase())
    } else {
        false
    }
}

pub struct LoadedEntity {
    pub key: EntityKey,
    pub entity: EntityPtr,
    pub serialized: String,
    pub version: u64,
    pub gid: u64,
}

pub struct EntityMap {
    entities: RefCell<HashMap<EntityKey, LoadedEntity>>,
}

impl EntityMap {
    pub fn new() -> Rc<Self> {
        Rc::new(Self {
            entities: RefCell::new(HashMap::new()),
        })
    }

    pub fn lookup_entity(&self, key: &EntityKey) -> Result<Option<EntityPtr>> {
        let check_existing = self.entities.borrow();
        if let Some(e) = check_existing.get(key) {
            debug!(%key, "existing");
            return Ok(Some(Rc::clone(&e.entity)));
        }

        Ok(None)
    }

    pub fn add_entity(&self, key: &EntityKey, loaded: LoadedEntity) -> Result<()> {
        let mut cache = self.entities.borrow_mut();

        cache.insert(key.clone(), loaded);

        Ok(())
    }

    pub fn foreach_entity<R, T: Fn(&LoadedEntity) -> Result<R>>(&self, each: T) -> Result<Vec<R>> {
        let cache = self.entities.borrow();

        let mut rvals: Vec<R> = Vec::new();

        for (_key, entity) in cache.iter() {
            rvals.push(each(entity)?);
        }

        Ok(rvals)
    }
}

struct Entities {
    entities: Rc<EntityMap>,
    storage: Rc<dyn EntityStorage>,
    infra: Weak<dyn Infrastructure>,
}

impl Debug for Entities {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Entities").finish()
    }
}

impl Entities {
    pub fn new(
        entities: Rc<EntityMap>,
        storage: Rc<dyn EntityStorage>,
        infra: Weak<dyn Infrastructure>,
    ) -> Rc<Self> {
        trace!("entities-new");

        Rc::new(Self {
            entities,
            storage,
            infra,
        })
    }

    // TODO Lots of cloning going on when adding new entities.
    fn add_entity(&self, entity: &EntityPtr) -> Result<()> {
        let loaded = {
            let clone = Rc::clone(entity);
            let entity = entity.borrow();
            LoadedEntity {
                key: entity.key.clone(),
                entity: clone,
                serialized: "".to_string(),
                version: 1,
                gid: 0,
            }
        };
        let key = loaded.key.clone();
        self.entities.add_entity(&key, loaded)
    }
}

impl PrepareEntities for Entities {
    fn prepare_entity_by_key(&self, key: &EntityKey) -> Result<EntityPtr> {
        if let Some(e) = self.entities.lookup_entity(key)? {
            return Ok(e);
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

        let cell = Rc::new(RefCell::new(loaded));

        self.entities.add_entity(
            key,
            LoadedEntity {
                key: key.clone(),
                entity: Rc::clone(&cell),
                serialized: persisted.serialized,
                gid: persisted.gid,
                version: persisted.version + 1,
            },
        )?;

        Ok(cell)
    }
}

#[derive(Debug)]
pub struct DomainInfrastructure {
    entities: Rc<Entities>,
}

impl DomainInfrastructure {
    pub fn new(storage: Rc<dyn EntityStorage>, entity_map: Rc<EntityMap>) -> Rc<Self> {
        Rc::new_cyclic(|me: &Weak<DomainInfrastructure>| {
            // How acceptable is this kind of thing?
            let infra = Weak::clone(me) as Weak<dyn Infrastructure>;
            let entities = Entities::new(entity_map, storage, infra);
            DomainInfrastructure { entities }
        })
    }
}

impl LoadEntities for DomainInfrastructure {
    fn load_entity_by_key(&self, key: &EntityKey) -> Result<EntityPtr> {
        self.entities.prepare_entity_by_key(key)
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

    fn add_entity(&self, entity: &EntityPtr) -> Result<()> {
        self.entities.add_entity(entity)
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
