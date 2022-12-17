use anyhow::{anyhow, Result};
use std::rc::Weak;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::{cell::RefCell, collections::HashMap, fmt::Debug, rc::Rc};
use tracing::*;

use super::{EntityRelationshipSet, Entry, IdentityFactory, KeySequence};
use crate::kernel::*;
use crate::storage::{EntityStorage, PersistedEntity};

#[derive(Debug)]
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
        info!("adding {:?}", loaded);

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
    fn perform(&self, user: &Entry, action: Box<dyn Action>) -> Result<Box<dyn Reply>>;
}

pub struct DomainInfrastructure {
    entities: Rc<Entities>,
    performer: Rc<dyn Performer>,
    keys: Arc<dyn KeySequence>,
    identities: Arc<dyn IdentityFactory>,
    raised: Rc<RefCell<Vec<Box<dyn DomainEvent>>>>,
    weak: Weak<DomainInfrastructure>,
}

impl DomainInfrastructure {
    pub fn new(
        storage: Rc<dyn EntityStorage>,
        entity_map: Rc<EntityMap>,
        performer: Rc<dyn Performer>,
        keys: Arc<dyn KeySequence>,
        identities: Arc<dyn IdentityFactory>,
        raised: Rc<RefCell<Vec<Box<dyn DomainEvent>>>>,
    ) -> Rc<Self> {
        let entities = Entities::new(entity_map, storage);
        Rc::new_cyclic(|weak| Self {
            entities,
            performer,
            keys,
            identities,
            raised,
            weak: Weak::clone(weak),
        })
    }

    fn find_item_in_set(
        &self,
        haystack: &EntityRelationshipSet,
        item: &Item,
    ) -> Result<Option<Entry>> {
        match item {
            Item::GID(gid) => {
                if let Some(e) = self.load_entity_by_gid(gid)? {
                    Ok(Some(e.try_into()?))
                } else {
                    Ok(None)
                }
            }
            _ => haystack.find_item(item),
        }
    }
}

impl Infrastructure for DomainInfrastructure {
    fn load_entity_by_key(&self, key: &EntityKey) -> Result<Option<EntityPtr>> {
        self.entities.prepare_entity_by_key(key)
    }

    fn load_entity_by_gid(&self, gid: &EntityGID) -> Result<Option<EntityPtr>> {
        self.entities.prepare_entity_by_gid(gid)
    }

    fn entry(&self, key: &EntityKey) -> Result<Option<Entry>> {
        match self.load_entity_by_key(key)? {
            Some(_) => Ok(Some(Entry {
                key: key.clone(),
                session: Weak::clone(&self.weak) as Weak<dyn Infrastructure>,
            })),
            None => Ok(None),
        }
    }

    fn find_item(&self, args: ActionArgs, item: &Item) -> Result<Option<Entry>> {
        let _loading_span = span!(Level::INFO, "finding", i = format!("{:?}", item)).entered();

        info!("finding");

        let haystack = EntityRelationshipSet::new_from_action(args).expand()?;

        self.find_item_in_set(&haystack, item)
    }
    fn ensure_entity(&self, entity_ref: &LazyLoadedEntity) -> Result<LazyLoadedEntity> {
        if entity_ref.has_entity() {
            Ok(entity_ref.clone())
        } else if let Some(entity) = self.load_entity_by_key(&entity_ref.key)? {
            Ok(entity.into())
        } else {
            Err(anyhow!("Entity not found"))
        }
    }

    fn add_entity(&self, entity: &EntityPtr) -> Result<Entry> {
        self.entities.add_entity(entity)?;

        Ok(self
            .entry(&entity.key())?
            .expect("Newly added entity has no Entry"))
    }

    fn chain(&self, living: &Entry, action: Box<dyn Action>) -> Result<Box<dyn Reply>> {
        self.performer.perform(living, action)
    }

    fn new_key(&self) -> EntityKey {
        self.keys.new_key()
    }

    fn new_identity(&self) -> Identity {
        self.identities.new_identity()
    }

    fn raise(&self, event: Box<dyn DomainEvent>) -> Result<()> {
        self.raised.borrow_mut().push(event);

        Ok(())
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
