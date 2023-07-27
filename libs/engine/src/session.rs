use anyhow::{anyhow, Result};
use std::rc::Weak;
use std::sync::Arc;
use std::time::Instant;
use std::{
    cell::RefCell,
    env,
    rc::Rc,
    sync::atomic::{AtomicBool, Ordering},
};
use tracing::{debug, info, span, trace, warn, Level};

use super::internal::{Entities, EntityMap, LoadedEntity};
use super::perform::StandardPerformer;
use super::sequences::{GlobalIds, Sequence};
use super::Notifier;
use crate::identifiers;
use crate::storage::{EntityStorage, PersistedEntity, PersistedFuture};
use kernel::*;

struct ModifiedEntity(PersistedEntity);

struct RaisedEvent {
    audience: Audience,
    event: Rc<dyn DomainEvent>,
}

pub struct Session {
    opened: Instant,
    open: AtomicBool,
    save_required: AtomicBool,
    storage: Rc<dyn EntityStorage>,
    ids: Rc<GlobalIds>,
    keys: Arc<dyn Sequence<EntityKey>>,
    identities: Arc<dyn Sequence<Identity>>,
    performer: Rc<StandardPerformer>,
    raised: Rc<RefCell<Vec<RaisedEvent>>>,
    weak: Weak<Session>,
    entities: Rc<Entities>,
    destroyed: RefCell<Vec<EntityKey>>,
    finder: Arc<dyn Finder>,
    plugins: Arc<RefCell<SessionPlugins>>,
    hooks: ManagedHooks,
}

impl Session {
    pub fn new(
        storage: Rc<dyn EntityStorage>,
        keys: &Arc<dyn Sequence<EntityKey>>,
        identities: &Arc<dyn Sequence<Identity>>,
        finder: &Arc<dyn Finder>,
        registered_plugins: &Arc<RegisteredPlugins>,
    ) -> Result<Rc<Self>> {
        trace!("session-new");

        let opened = Instant::now();

        storage.begin()?;

        let plugins = registered_plugins.create_plugins()?;
        let plugins = Arc::new(RefCell::new(plugins));

        let hooks = {
            let plugins = plugins.borrow();
            plugins.hooks()?
        };

        let middleware: Rc<Vec<Rc<dyn Middleware>>> = {
            let mut plugins = plugins.borrow_mut();
            Rc::new(plugins.middleware()?)
        };

        let ids = GlobalIds::new();
        let entities = Entities::new(EntityMap::new());
        let session = Rc::new_cyclic(|weak: &Weak<Session>| Self {
            opened,
            storage,
            entities,
            ids,
            hooks,
            weak: Weak::clone(weak),
            open: AtomicBool::new(true),
            save_required: AtomicBool::new(false),
            performer: StandardPerformer::new(
                weak,
                Arc::clone(finder),
                Arc::clone(&plugins),
                middleware,
                None,
            ),
            raised: Rc::new(RefCell::new(Vec::new())),
            keys: Arc::clone(keys),
            identities: Arc::clone(identities),
            destroyed: RefCell::new(Vec::new()),
            finder: Arc::clone(finder),
            plugins,
        });

        session.set_session()?;

        Ok(session)
    }

    pub fn world(&self) -> Result<Entry, DomainError> {
        self.entry(&LookupBy::Key(&WORLD_KEY.into()))?
            .ok_or(DomainError::EntityNotFound)
    }

    pub fn evaluate_and_perform(&self, user_name: &str, text: &str) -> Result<Option<Effect>> {
        if !self.open.load(Ordering::Relaxed) {
            return Err(DomainError::SessionClosed.into());
        }

        match self.performer.evaluate_and_perform(user_name, text) {
            Ok(i) => Ok(i),
            Err(original_err) => {
                if let Err(_rollback_err) = self.storage.rollback(false) {
                    // TODO Include that this failed as part of the error.
                    panic!("TODO error rolling back");
                }

                self.open.store(false, Ordering::Relaxed);

                Err(original_err)
            }
        }
    }

    pub fn flush(&self) -> Result<()> {
        self.save_entity_changes()?;
        self.storage.begin()
    }

    pub fn deliver(&self, incoming: Incoming) -> Result<()> {
        let plugins = self.plugins.borrow();

        plugins.deliver(incoming)?;

        Ok(())
    }

    fn set_session(&self) -> Result<()> {
        let session: Rc<dyn ActiveSession> = self.weak.upgrade().ok_or(DomainError::NoSession)?;
        set_my_session(Some(&session))?;

        self.initialize()?;

        Ok(())
    }

    fn initialize(&self) -> Result<()> {
        if let Some(gid) = self.get_gid()? {
            self.ids.set(&gid);
        }

        let mut plugins = self.plugins.borrow_mut();
        plugins.initialize()?;

        Ok(())
    }

    fn get_gid(&self) -> Result<Option<EntityGid>> {
        if let Some(world) = self.entry(&LookupBy::Key(&WORLD_KEY.into()))? {
            identifiers::model::get_gid(&world)
        } else {
            Ok(None)
        }
    }

    fn save_entity_changes(&self) -> Result<()> {
        self.save_modified_ids()?;

        let destroyed = self.destroyed.borrow();

        let saves = SavesEntities {
            storage: &self.storage,
            destroyed: &destroyed,
        };
        let changes = saves.save_modified_entities(&self.entities)?;
        let required = self.save_required.load(Ordering::SeqCst);

        if changes || required {
            // Check for a force rollback, usually debugging purposes.
            if should_force_rollback() {
                let _span = span!(Level::DEBUG, "FORCED").entered();
                self.storage.rollback(true)
            } else {
                self.storage.commit()
            }
        } else {
            self.storage.rollback(true)
        }
    }

    pub fn close<T: Notifier>(&self, notifier: &T) -> Result<()> {
        self.save_entity_changes()?;

        self.flush_raised(notifier)?;

        let nentities = self.entities.size();
        let elapsed = self.opened.elapsed();
        let elapsed = format!("{:?}", elapsed);

        let plugins = self.plugins.borrow();

        plugins.stop()?;

        info!(%elapsed, %nentities, "closed");

        self.open.store(false, Ordering::Relaxed);

        Ok(())
    }

    fn flush_raised<T: Notifier>(&self, notifier: &T) -> Result<()> {
        let mut pending = self.raised.borrow_mut();
        let npending = pending.len();
        if npending == 0 {
            return Ok(());
        }

        info!(%npending, "raising");

        for raised in pending.iter() {
            debug!("{:?}", raised.event);
            debug!("{:?}", raised.event.to_json()?);
            let audience_keys = self.finder.find_audience(&raised.audience)?;
            for key in audience_keys {
                notifier.notify(&key, &raised.event)?;
            }
        }

        pending.clear();

        Ok(())
    }

    fn save_modified_ids(&self) -> Result<()> {
        // Check to see if the global identifier has changed due to the creation
        // of a new entity.
        let world = self.world()?;
        let previous_gid =
            identifiers::model::get_gid(&world)?.unwrap_or_else(|| EntityGid::new(0));
        let new_gid = self.ids.gid();
        if previous_gid != new_gid {
            info!(%previous_gid, %new_gid, "gid:changed");
            identifiers::model::set_gid(&world, new_gid)?;
        } else {
            debug!(gid = %previous_gid, "gid:same");
        }

        Ok(())
    }
}

struct SavesEntities<'a> {
    storage: &'a Rc<dyn EntityStorage>,
    destroyed: &'a Vec<EntityKey>,
}

impl<'a> SavesEntities<'a> {
    fn check_for_changes(&self, l: &mut LoadedEntity) -> Result<Option<ModifiedEntity>> {
        use kernel::compare::*;

        let _span = span!(Level::TRACE, "flushing", key = l.key.to_string()).entered();

        if let Some(modified) = any_entity_changes(AnyChanges {
            before: l.serialized.as_ref().map(Original::String),
            after: l.entity.clone(),
        })? {
            // Serialize to string now that we know we'll use this.
            let serialized = modified.after.to_string();

            // By now we should have a global identifier.
            let Some(gid) = l.gid.clone() else  {
                return Err(anyhow!("Expected EntityGid in check_for_changes"));
            };

            let previous = l.version;
            l.version += 1;

            Ok(Some(ModifiedEntity(PersistedEntity {
                key: l.key.to_string(),
                gid: gid.into(),
                version: previous,
                serialized,
            })))
        } else {
            Ok(None)
        }
    }

    fn save_entity(&self, modified: &ModifiedEntity) -> Result<()> {
        if self.is_deleted(&EntityKey::new(&modified.0.key)) {
            self.storage.delete(&modified.0)
        } else {
            self.storage.save(&modified.0)
        }
    }

    fn is_deleted(&self, key: &EntityKey) -> bool {
        self.destroyed.contains(key)
    }

    fn save_modified_entities(&self, entities: &Entities) -> Result<bool> {
        Ok(!self
            .get_modified_entities(entities)?
            .into_iter()
            .map(|modified| self.save_entity(&modified))
            .collect::<Result<Vec<_>>>()?
            .is_empty())
    }

    fn get_modified_entities(&self, entities: &Entities) -> Result<Vec<ModifiedEntity>> {
        let modified = entities.foreach_entity_mut(|l| self.check_for_changes(l))?;
        Ok(modified.into_iter().flatten().collect::<Vec<_>>())
    }
}

impl LoadsEntities for Session {
    fn load_entity(&self, lookup: &LookupBy) -> Result<Option<EntityPtr>> {
        if let Some(e) = self.entities.lookup_entity(lookup)? {
            return Ok(Some(e));
        }

        let _loading_span =
            span!(Level::INFO, "entity", lookup = format!("{:?}", lookup)).entered();

        trace!("loading");
        if let Some(persisted) = self.storage.load(lookup)? {
            Ok(Some(self.entities.add_persisted(persisted)?))
        } else {
            Ok(None)
        }
    }
}

impl ActiveSession for Session {
    fn entry(&self, lookup: &LookupBy) -> Result<Option<Entry>> {
        match self.load_entity(lookup)? {
            Some(entity) => Ok(Some(Entry::new(
                &entity.key(),
                entity,
                Weak::clone(&self.weak) as Weak<dyn ActiveSession>,
            ))),
            None => Ok(None),
        }
    }

    fn find_item(&self, surroundings: &Surroundings, item: &Item) -> Result<Option<Entry>> {
        let _loading_span = span!(Level::INFO, "finding", i = format!("{:?}", item)).entered();

        info!("finding");

        match item {
            Item::Gid(gid) => self.entry(&LookupBy::Gid(gid)),
            _ => self.finder.find_item(surroundings, item),
        }
    }

    fn ensure_entity(&self, entity_ref: &EntityRef) -> Result<EntityRef, DomainError> {
        if entity_ref.has_entity() {
            Ok(entity_ref.clone())
        } else if let Some(entity) = &self.load_entity(&LookupBy::Key(entity_ref.key()))? {
            Ok(entity.into())
        } else {
            Err(DomainError::EntityNotFound)
        }
    }

    fn add_entity(&self, entity: &EntityPtr) -> Result<Entry> {
        self.entities.add_entity(&self.ids, entity)?;

        Ok(self
            .entry(&LookupBy::Key(&entity.key()))?
            .expect("Bug: Newly added entity has no Entry"))
    }

    fn obliterate(&self, entry: &Entry) -> Result<()> {
        let destroying = entry.entity();
        let mut destroying = destroying.borrow_mut();
        destroying.destroy()?;

        self.destroyed.borrow_mut().push(entry.key().clone());

        Ok(())
    }

    fn chain(&self, perform: Perform) -> Result<Effect> {
        self.performer.perform(perform)
    }

    fn new_key(&self) -> EntityKey {
        self.keys.following()
    }

    fn new_identity(&self) -> Identity {
        self.identities.following()
    }

    fn raise(&self, audience: Audience, event: Box<dyn DomainEvent>) -> Result<()> {
        self.raised.borrow_mut().push(RaisedEvent {
            audience,
            event: event.into(),
        });

        Ok(())
    }

    fn hooks(&self) -> &ManagedHooks {
        &self.hooks
    }

    fn schedule(&self, key: &str, when: When, message: &dyn ToJson) -> Result<()> {
        let key = key.to_owned();
        let time = when.to_utc_time()?;
        let serialized = message.to_json()?.to_string();
        let future = PersistedFuture {
            key,
            time,
            serialized,
        };

        self.storage.queue(future)?;

        self.save_required.store(true, Ordering::SeqCst);

        Ok(())
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // This feels like the most defensive solution. If there's ever a reason
        // this can happen we can make this warn.
        set_my_session(None).expect("Error clearing session");

        if self.open.load(Ordering::Relaxed) {
            warn!("session-drop: open session!");
        } else {
            trace!("session-drop");
        }
    }
}

fn should_force_rollback() -> bool {
    env::var("FORCE_ROLLBACK").is_ok()
}
