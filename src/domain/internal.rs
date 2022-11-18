use anyhow::{anyhow, Result};
use std::sync::atomic::{AtomicI64, Ordering};
use std::{cell::RefCell, collections::HashMap, fmt::Debug, rc::Rc};
use tracing::{debug, info, span, trace, Level};

use crate::storage::{EntityStorage, PersistedEntity};
use crate::{kernel::*, plugins::carrying::model::Containing};

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

pub struct LoadedEntity {
    pub key: EntityKey,
    pub entity: EntityPtr,
    pub version: u64,
    pub gid: Option<EntityGID>,
    pub serialized: Option<String>,
}

pub struct EntityMap {
    ids: Rc<GlobalIds>,
    by_key: RefCell<HashMap<EntityKey, LoadedEntity>>,
    by_gid: RefCell<HashMap<EntityGID, EntityKey>>,
}

impl EntityMap {
    pub fn new(ids: Rc<GlobalIds>) -> Rc<Self> {
        Rc::new(Self {
            ids,
            by_key: RefCell::new(HashMap::new()),
            by_gid: RefCell::new(HashMap::new()),
        })
    }

    pub fn size(&self) -> usize {
        self.by_key.borrow().len()
    }

    pub fn lookup_entity_by_key(&self, key: &EntityKey) -> Result<Option<EntityPtr>> {
        let check_existing = self.by_key.borrow();
        if let Some(e) = check_existing.get(key) {
            trace!(%key, "existing");
            return Ok(Some(e.entity.clone()));
        }

        Ok(None)
    }

    pub fn lookup_entity_by_gid(&self, gid: &EntityGID) -> Result<Option<EntityPtr>> {
        let check_existing = self.by_gid.borrow();
        if let Some(k) = check_existing.get(gid) {
            Ok(self.lookup_entity_by_key(k)?)
        } else {
            Ok(None)
        }
    }

    pub fn add_entity(&self, mut loaded: LoadedEntity) -> Result<()> {
        let mut key_cache = self.by_key.borrow_mut();
        let mut gid_cache = self.by_gid.borrow_mut();
        let key = loaded.key.clone();

        let gid = match &loaded.gid {
            Some(gid) => gid.clone(),
            None => {
                let gid = self.ids.get();
                let loaded = &mut loaded;
                loaded.gid = Some(gid.clone());
                gid
            }
        };

        gid_cache.insert(gid, key.clone());
        key_cache.insert(key, loaded);

        Ok(())
    }

    pub fn foreach_entity<R, T: Fn(&LoadedEntity) -> Result<R>>(&self, each: T) -> Result<Vec<R>> {
        let cache = self.by_key.borrow();

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

        let gid = loaded.gid().clone();
        let cell: EntityPtr = loaded.into();

        self.entities.add_entity(LoadedEntity {
            key: EntityKey::new(&persisted.key),
            entity: cell.clone(),
            version: persisted.version + 1,
            gid: gid,
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
}

impl DomainInfrastructure {
    pub fn new(
        storage: Rc<dyn EntityStorage>,
        entity_map: Rc<EntityMap>,
        performer: Rc<dyn Performer>,
    ) -> Rc<Self> {
        let entities = Entities::new(entity_map, storage);
        Rc::new(DomainInfrastructure {
            entities,
            performer,
        })
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

    fn find_item(&self, args: ActionArgs, item: &Item) -> Result<Option<EntityPtr>> {
        let _loading_span = span!(Level::INFO, "finding", i = format!("{:?}", item)).entered();

        info!("finding");

        match item {
            Item::Named(name) => {
                let haystack = EntityRelationshipSet::new_from_action(args).expand()?;

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
                        _ => {}
                    }
                }

                Ok(None)
            }
            Item::Route(name) => {
                let haystack = EntityRelationshipSet::new_from_action(args)
                    .expand()?
                    .routes()?;

                debug!("route:haystack {:?}", haystack);

                for entity in &haystack.entities {
                    match entity {
                        EntityRelationship::World(_) => {}
                        EntityRelationship::User(_) => {}
                        EntityRelationship::Area(_) => {}
                        EntityRelationship::Holding(_) => {}
                        EntityRelationship::Ground(_) => {}
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
        }
    }

    fn add_entity(&self, entity: &EntityPtr) -> Result<()> {
        self.entities.add_entity(entity)
    }

    fn chain(&self, living: &EntityPtr, action: Box<dyn Action>) -> Result<Box<dyn Reply>> {
        self.performer.perform(living, action)
    }
}

#[derive(Debug, Clone)]
enum EntityRelationship {
    World(EntityPtr),
    User(EntityPtr),
    Area(EntityPtr),
    Holding(EntityPtr),
    Ground(EntityPtr),
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
                EntityRelationship::World(_world) => {}
                EntityRelationship::Holding(_holding) => {}
                EntityRelationship::Ground(_ground) => {}
                EntityRelationship::Exit(_route_name, _area) => {}
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
    gid: AtomicI64,
}

impl GlobalIds {
    pub fn new() -> Rc<Self> {
        Rc::new(Self {
            gid: AtomicI64::new(0),
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
