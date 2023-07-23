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
        let ids = GlobalIds::new();
        let entity_map = EntityMap::new(Rc::clone(&ids));

        storage.begin()?;

        let plugins = registered_plugins.create_plugins()?;
        let plugins = Arc::new(RefCell::new(plugins));

        let hooks = {
            let plugins = plugins.borrow();
            plugins.hooks()?
        };

        let session = Rc::new_cyclic(|weak: &Weak<Session>| Self {
            opened,
            storage: Rc::clone(&storage),
            open: AtomicBool::new(true),
            save_required: AtomicBool::new(false),
            performer: StandardPerformer::new(weak, Arc::clone(finder), Arc::clone(&plugins)),
            ids: Rc::clone(&ids),
            raised: Rc::new(RefCell::new(Vec::new())),
            weak: Weak::clone(weak),
            entities: Entities::new(entity_map, storage),
            keys: Arc::clone(keys),
            identities: Arc::clone(identities),
            destroyed: RefCell::new(Vec::new()),
            finder: Arc::clone(finder),
            plugins: Arc::clone(&plugins),
            hooks,
        });

        session.set_session()?;

        if let Some(world) = session.entry(&LookupBy::Key(&WORLD_KEY.into()))? {
            if let Some(gid) = identifiers::model::get_gid(&world)? {
                ids.set(&gid);
            }
        }

        session.initialize()?;

        Ok(session)
    }

    fn set_session(&self) -> Result<()> {
        let session: Rc<dyn ActiveSession> = self.weak.upgrade().ok_or(DomainError::NoSession)?;
        set_my_session(Some(&session))?;

        Ok(())
    }

    fn initialize(&self) -> Result<()> {
        let mut plugins = self.plugins.borrow_mut();

        plugins.initialize()?;

        Ok(())
    }

    pub fn world(&self) -> Result<Entry, DomainError> {
        self.entry(&LookupBy::Key(&WORLD_KEY.into()))?
            .ok_or(DomainError::EntityNotFound)
    }

    pub fn find_name_key(&self, user_name: &str) -> Result<Option<EntityKey>, DomainError> {
        if !self.open.load(Ordering::Relaxed) {
            return Err(DomainError::SessionClosed);
        }

        self.performer.find_name_key(user_name)
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

    pub fn deliver(&self, incoming: Incoming) -> Result<()> {
        let plugins = self.plugins.borrow();

        plugins.deliver(incoming)?;

        Ok(())
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

    fn check_for_changes(&self, l: &mut LoadedEntity) -> Result<Option<ModifiedEntity>> {
        use kernel::compare::*;

        let _span = span!(Level::TRACE, "flushing", key = l.key.to_string()).entered();

        if let Some(modified) = any_entity_changes(AnyChanges {
            entity: &l.entity,
            original: l.serialized.as_ref().map(Original::String),
        })? {
            // Serialize to string now that we know we'll use this.
            let serialized = modified.entity.to_string();

            // By now we should have a global identifier.
            if l.gid.is_none() {
                return Err(anyhow!("Expected EntityGid in check_for_changes"));
            }
            let gid = l.gid.clone().unwrap();

            // I'm on the look out for a better way to handle this. Part of me
            // wishes that it was done after the save and that part is at odds
            // with the part of me that says here is fine because if the save
            // fails all bets are off anyway. Also the odds of us ever trying to
            // recover from a failed save are very low. Easier to just repeat.
            let version_being_saved = l.version;
            l.version += 1;

            {
                // It would be nice if there was a way to do this in a way that
                // didn't expose these methods. I believe they're a smell, just
                // need a solution.  It would also be nice if we could do this
                // and some of the above syncing later, after the save is known
                // to be good, but I digress.
                let mut entity = l.entity.borrow_mut();
                entity.set_gid(gid.clone())?;
                entity.set_version(l.version)?;
            }

            Ok(Some(ModifiedEntity(PersistedEntity {
                key: l.key.to_string(),
                gid: gid.into(),
                version: version_being_saved,
                serialized,
            })))
        } else {
            Ok(None)
        }
    }

    fn save_entity_changes(&self) -> Result<()> {
        // We have to do this before checking for modifications so that the
        // state in the world Entity gets saved.
        self.save_modified_ids()?;

        if self.save_modified_entities()? || self.save_required.load(Ordering::SeqCst) {
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

    fn save_entity(&self, modified: &ModifiedEntity) -> Result<()> {
        if self.is_deleted(&EntityKey::new(&modified.0.key)) {
            self.storage.delete(&modified.0)
        } else {
            self.storage.save(&modified.0)
        }
    }

    fn is_deleted(&self, key: &EntityKey) -> bool {
        self.destroyed.borrow().contains(key)
    }

    fn save_modified_entities(&self) -> Result<bool> {
        Ok(!self
            .get_modified_entities()?
            .into_iter()
            .map(|modified| self.save_entity(&modified))
            .collect::<Result<Vec<_>>>()?
            .is_empty())
    }

    fn get_modified_entities(&self) -> Result<Vec<ModifiedEntity>> {
        let modified = self
            .entities
            .foreach_entity_mut(|l| self.check_for_changes(l))?;
        Ok(modified.into_iter().flatten().collect::<Vec<_>>())
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

    fn load_entity(&self, lookup: &LookupBy) -> Result<Option<EntityPtr>> {
        self.entities.prepare_entity(lookup)
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
        self.entities.add_entity(entity)?;

        Ok(self
            .entry(&LookupBy::Key(&entity.key()))?
            .expect("Bug: Newly added entity has no Entry"))
    }

    fn obliterate(&self, entry: &Entry) -> Result<()> {
        let destroying = entry.entity()?;
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
