use anyhow::{anyhow, Result};
use std::sync::atomic::{AtomicU64, Ordering};
use std::{cell::RefCell, collections::HashMap, fmt::Debug, rc::Rc};
use tracing::{debug, info, span, trace, Level};

use crate::kernel::*;
use crate::plugins::tools;
use crate::storage::{EntityStorage, PersistedEntity};

use super::KeySequence;

pub struct LoadedEntity {
    pub key: EntityKey,
    pub entity: EntityPtr,
    pub version: u64,
    pub gid: Option<EntityGID>,
    pub serialized: Option<String>,
}

struct Maps {
    by_key: HashMap<EntityKey, LoadedEntity>,
    by_gid: HashMap<EntityGID, EntityKey>,
}

impl Maps {
    fn new() -> Self {
        Maps {
            by_key: HashMap::new(),
            by_gid: HashMap::new(),
        }
    }

    fn size(&self) -> usize {
        self.by_key.len()
    }

    fn lookup_entity_by_key(&self, key: &EntityKey) -> Result<Option<EntityPtr>> {
        if let Some(e) = self.by_key.get(key) {
            trace!(%key, "existing");
            Ok(Some(e.entity.clone()))
        } else {
            Ok(None)
        }
    }

    fn lookup_entity_by_gid(&self, gid: &EntityGID) -> Result<Option<EntityPtr>> {
        if let Some(k) = self.by_gid.get(gid) {
            Ok(self.lookup_entity_by_key(k)?)
        } else {
            Ok(None)
        }
    }

    fn add_entity(&mut self, loaded: LoadedEntity) -> Result<()> {
        self.by_gid.insert(
            loaded
                .gid
                .clone()
                .ok_or_else(|| anyhow!("Entity missing GID"))?,
            loaded.key.clone(),
        );
        self.by_key.insert(loaded.key.clone(), loaded);

        Ok(())
    }

    fn foreach_entity_mut<R, T: Fn(&mut LoadedEntity) -> Result<R>>(
        &mut self,
        each: T,
    ) -> Result<Vec<R>> {
        let mut rvals: Vec<R> = Vec::new();

        for (_key, entity) in self.by_key.iter_mut() {
            rvals.push(each(entity)?);
        }

        Ok(rvals)
    }

    fn foreach_entity<R, T: Fn(&LoadedEntity) -> Result<R>>(&self, each: T) -> Result<Vec<R>> {
        let mut rvals: Vec<R> = Vec::new();

        for (_key, entity) in self.by_key.iter() {
            rvals.push(each(entity)?);
        }

        Ok(rvals)
    }
}

pub struct EntityMap {
    ids: Rc<GlobalIds>,
    maps: RefCell<Maps>,
}

impl EntityMap {
    pub fn new(ids: Rc<GlobalIds>) -> Rc<Self> {
        Rc::new(Self {
            ids,
            maps: RefCell::new(Maps::new()),
        })
    }

    pub fn size(&self) -> usize {
        self.maps.borrow().size()
    }

    pub fn lookup_entity_by_key(&self, key: &EntityKey) -> Result<Option<EntityPtr>> {
        self.maps.borrow().lookup_entity_by_key(key)
    }

    pub fn lookup_entity_by_gid(&self, gid: &EntityGID) -> Result<Option<EntityPtr>> {
        self.maps.borrow().lookup_entity_by_gid(gid)
    }

    pub fn add_entity(&self, mut loaded: LoadedEntity) -> Result<()> {
        self.assign_gid_if_necessary(&mut loaded);
        self.maps.borrow_mut().add_entity(loaded)
    }

    fn assign_gid_if_necessary(&self, mut loaded: &mut LoadedEntity) {
        match &loaded.gid {
            Some(gid) => gid.clone(),
            None => {
                let gid = self.ids.get();
                let loaded = &mut loaded;
                loaded.gid = Some(gid.clone());
                gid
            }
        };
    }

    pub fn foreach_entity_mut<R, T: Fn(&mut LoadedEntity) -> Result<R>>(
        &self,
        each: T,
    ) -> Result<Vec<R>> {
        self.maps.borrow_mut().foreach_entity_mut(each)
    }

    #[allow(dead_code)]
    pub fn foreach_entity<R, T: Fn(&LoadedEntity) -> Result<R>>(&self, each: T) -> Result<Vec<R>> {
        self.maps.borrow().foreach_entity(each)
    }
}

struct Entities {
    entities: Rc<EntityMap>,
    storage: Rc<dyn EntityStorage>,
}

impl Debug for Entities {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Entities").finish()
    }
}

impl Entities {
    pub fn new(entities: Rc<EntityMap>, storage: Rc<dyn EntityStorage>) -> Rc<Self> {
        trace!("entities-new");

        Rc::new(Self { entities, storage })
    }

    fn add_entity(&self, entity: &EntityPtr) -> Result<()> {
        let clone = entity.clone();
        let entity = entity.borrow();
        self.entities.add_entity(LoadedEntity {
            key: entity.key.clone(),
            entity: clone,
            serialized: None,
            version: 1,
            gid: entity.gid(),
        })
    }

    fn prepare_persisted(&self, persisted: PersistedEntity) -> Result<EntityPtr> {
        trace!("parsing");
        let mut loaded: Entity = serde_json::from_str(&persisted.serialized)?;

        trace!("infrastructure");
        let session = get_my_session()?; // Thread local session!
        loaded.supply(&session)?;

        let gid = loaded.gid();
        let cell: EntityPtr = loaded.into();

        self.entities.add_entity(LoadedEntity {
            key: EntityKey::new(&persisted.key),
            entity: cell.clone(),
            version: persisted.version + 1,
            gid,
            serialized: Some(persisted.serialized),
        })?;

        Ok(cell)
    }

    fn prepare_entity_by_key(&self, key: &EntityKey) -> Result<Option<EntityPtr>> {
        if let Some(e) = self.entities.lookup_entity_by_key(key)? {
            return Ok(Some(e));
        }

        let _loading_span = span!(Level::INFO, "entity", key = key.key_to_string()).entered();

        info!("loading");
        if let Some(persisted) = self.storage.load_by_key(key)? {
            Ok(Some(self.prepare_persisted(persisted)?))
        } else {
            Ok(None)
        }
    }

    fn prepare_entity_by_gid(&self, gid: &EntityGID) -> Result<Option<EntityPtr>> {
        if let Some(e) = self.entities.lookup_entity_by_gid(gid)? {
            return Ok(Some(e));
        }

        let _loading_span = span!(Level::INFO, "entity", gid = gid.gid_to_string()).entered();

        info!("loading");
        if let Some(persisted) = self.storage.load_by_gid(gid)? {
            Ok(Some(self.prepare_persisted(persisted)?))
        } else {
            Ok(None)
        }
    }
}

pub trait Performer {
    fn perform(&self, user: &EntityPtr, action: Box<dyn Action>) -> Result<Box<dyn Reply>>;
}

pub struct DomainInfrastructure {
    entities: Rc<Entities>,
    performer: Rc<dyn Performer>,
    keys: Rc<dyn KeySequence>,
}

impl DomainInfrastructure {
    pub fn new(
        storage: Rc<dyn EntityStorage>,
        entity_map: Rc<EntityMap>,
        performer: Rc<dyn Performer>,
        keys: Rc<dyn KeySequence>,
    ) -> Rc<Self> {
        let entities = Entities::new(entity_map, storage);
        Rc::new(DomainInfrastructure {
            entities,
            performer,
            keys,
        })
    }

    fn find_item_in_set(
        &self,
        haystack: &EntityRelationshipSet,
        item: &Item,
    ) -> Result<Option<EntityPtr>> {
        match item {
            Item::Named(name) => {
                debug!("item:haystack {:?}", haystack);

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
                        EntityRelationship::Contained(e) => {
                            if matches_description(&e.borrow(), name) {
                                return Ok(Some(e.clone()));
                            }
                        }
                        _ => {}
                    }
                }

                Ok(None)
            }
            Item::Route(name) => {
                let haystack = haystack.routes()?;

                debug!("route:haystack {:?}", haystack);

                for entity in &haystack.entities {
                    match entity {
                        EntityRelationship::World(_) => {}
                        EntityRelationship::User(_) => {}
                        EntityRelationship::Area(_) => {}
                        EntityRelationship::Holding(_) => {}
                        EntityRelationship::Ground(_) => {}
                        EntityRelationship::Contained(_) => {}
                        EntityRelationship::Exit(route_name, area) => {
                            if matches_string_description(route_name, name) {
                                info!("found: {:?} -> {:?}", route_name, area);
                                return Ok(Some(area.clone()));
                            }
                        }
                    }
                }

                Ok(None)
            }
            Item::GID(gid) => {
                if let Some(e) = self.load_entity_by_gid(gid)? {
                    Ok(Some(e))
                } else {
                    Ok(None)
                }
            }
            // Item::Held(_) => todo!(),
            Item::Contained(contained) => {
                let haystack = haystack.expand()?;

                self.find_item_in_set(&haystack, contained)
            }
        }
    }
}

impl LoadEntities for DomainInfrastructure {
    fn load_entity_by_key(&self, key: &EntityKey) -> Result<Option<EntityPtr>> {
        self.entities.prepare_entity_by_key(key)
    }

    fn load_entity_by_gid(&self, gid: &EntityGID) -> Result<Option<EntityPtr>> {
        self.entities.prepare_entity_by_gid(gid)
    }
}

fn matches_string_description(incoming: &str, desc: &str) -> bool {
    // TODO We can do this more efficiently.
    incoming.to_lowercase().contains(&desc.to_lowercase())
}

/// Determines if an entity matches a user's description of that entity, given
/// no other context at all.
fn matches_description(entity: &Entity, desc: &str) -> bool {
    if let Some(name) = entity.name() {
        matches_string_description(&name, desc)
    } else {
        false
    }
}

impl FindsItems for DomainInfrastructure {
    fn find_item(&self, args: ActionArgs, item: &Item) -> Result<Option<EntityPtr>> {
        let _loading_span = span!(Level::INFO, "finding", i = format!("{:?}", item)).entered();

        info!("finding");

        let haystack = EntityRelationshipSet::new_from_action(args).expand()?;

        self.find_item_in_set(&haystack, item)
    }
}

impl Infrastructure for DomainInfrastructure {
    fn ensure_entity(&self, entity_ref: &LazyLoadedEntity) -> Result<LazyLoadedEntity> {
        if entity_ref.has_entity() {
            Ok(entity_ref.clone())
        } else if let Some(entity) = self.load_entity_by_key(&entity_ref.key)? {
            Ok(entity.into())
        } else {
            Err(anyhow!("Entity not found"))
        }
    }

    fn add_entity(&self, entity: &EntityPtr) -> Result<()> {
        self.entities.add_entity(entity)
    }

    fn chain(&self, living: &EntityPtr, action: Box<dyn Action>) -> Result<Box<dyn Reply>> {
        self.performer.perform(living, action)
    }

    fn new_key(&self) -> EntityKey {
        self.keys.new_key()
    }
}

#[derive(Debug, Clone)]
enum EntityRelationship {
    World(EntityPtr),
    User(EntityPtr),
    Area(EntityPtr),
    Holding(EntityPtr),
    Ground(EntityPtr),
    /// Items is nearby, inside something else. Considering renaming this and
    /// others to better indicate how far removed they are. For example,
    /// containers in the area vs containers that are being held.
    Contained(EntityPtr),
    Exit(String, EntityPtr),
}

#[derive(Debug)]
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
                EntityRelationship::User(user) => expanded.extend(
                    tools::contained_by(user)?
                        .into_iter()
                        .map(EntityRelationship::Holding)
                        .collect::<Vec<_>>(),
                ),
                EntityRelationship::Area(area) => expanded.extend(
                    tools::contained_by(area)?
                        .into_iter()
                        .map(EntityRelationship::Ground)
                        .collect::<Vec<_>>(),
                ),
                EntityRelationship::World(_world) => {}
                EntityRelationship::Holding(holding) => expanded.extend(
                    tools::contained_by(holding)?
                        .into_iter()
                        .map(EntityRelationship::Contained)
                        .collect::<Vec<_>>(),
                ),
                EntityRelationship::Ground(_ground) => {}
                EntityRelationship::Exit(_route_name, _area) => {}
                EntityRelationship::Contained(_) => {}
            }
        }

        Ok(Self { entities: expanded })
    }

    pub fn routes(&self) -> Result<Self> {
        use crate::plugins::moving::model::Exit;

        let mut expanded = self.entities.clone();

        // https://github.com/ferrous-systems/elements-of-rust#tuple-structs-and-enum-tuple-variants-as-functions
        for entity in &self.entities {
            if let EntityRelationship::Ground(ground) = entity {
                let item = ground.borrow();
                if let Some(exit) = item.maybe_scope::<Exit>()? {
                    expanded.push(EntityRelationship::Exit(
                        item.name()
                            .ok_or_else(|| anyhow!("Route name is required"))?,
                        exit.area.into_entity()?,
                    ));
                }
            }
        }

        Ok(Self { entities: expanded })
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

    pub fn gid(&self) -> EntityGID {
        EntityGID::new(self.gid.load(Ordering::Relaxed))
    }

    pub fn set(&self, gid: &EntityGID) {
        self.gid.store(gid.into(), Ordering::Relaxed);
    }

    pub fn get(&self) -> EntityGID {
        EntityGID::new(self.gid.fetch_add(1, Ordering::Relaxed) + 1)
    }
}
