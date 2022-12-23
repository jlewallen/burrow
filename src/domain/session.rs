use anyhow::Result;
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

use super::internal::{Entities, EntityMap, GlobalIds, LoadedEntity};
use super::perform::StandardPerformer;
use super::{EntityRelationshipSet, Notifier, Sequence};
use crate::kernel::*;
use crate::plugins::identifiers;
use crate::plugins::tools;
use crate::storage::{EntityStorage, PersistedEntity};

struct ModifiedEntity(PersistedEntity);

pub struct Session {
    opened: Instant,
    open: AtomicBool,
    storage: Rc<dyn EntityStorage>,
    ids: Rc<GlobalIds>,
    performer: Rc<StandardPerformer>,
    raised: Rc<RefCell<Vec<Box<dyn DomainEvent>>>>,
    weak: Weak<Session>,
    entities: Rc<Entities>,
    keys: Arc<dyn Sequence<EntityKey>>,
    identities: Arc<dyn Sequence<Identity>>,
    destroyed: RefCell<Vec<EntityKey>>,
}

impl Session {
    pub fn new(
        storage: Rc<dyn EntityStorage>,
        keys: &Arc<dyn Sequence<EntityKey>>,
        identities: &Arc<dyn Sequence<Identity>>,
    ) -> Result<Rc<Self>> {
        trace!("session-new");

        let opened = Instant::now();
        let ids = GlobalIds::new();
        let entity_map = EntityMap::new(Rc::clone(&ids));
        let raised = Rc::new(RefCell::new(Vec::new()));

        storage.begin()?;

        let session = Rc::new_cyclic(|weak: &Weak<Session>| Self {
            opened,
            storage: Rc::clone(&storage),
            open: AtomicBool::new(true),
            performer: StandardPerformer::new(weak),
            ids: Rc::clone(&ids),
            raised,
            weak: Weak::clone(weak),
            entities: Entities::new(entity_map, storage),
            keys: Arc::clone(keys),
            identities: Arc::clone(identities),
            destroyed: RefCell::new(Vec::new()),
        });

        session.set_session()?;

        if let Some(world) = session.entry(&WORLD_KEY)? {
            if let Some(gid) = identifiers::model::get_gid(&world)? {
                ids.set(&gid);
            }
        }

        Ok(session)
    }

    fn set_session(&self) -> Result<()> {
        let infra: Rc<dyn Infrastructure> =
            self.weak.upgrade().ok_or(DomainError::NoInfrastructure)?;
        set_my_session(Some(&infra))?;

        Ok(())
    }

    pub fn entry(&self, key: &EntityKey) -> Result<Option<Entry>> {
        match self.load_entity_by_key(key)? {
            Some(entity) => Ok(Some(Entry {
                key: key.clone(),
                entity: entity,
                session: Weak::clone(&self.weak) as Weak<dyn Infrastructure>,
            })),
            None => Ok(None),
        }
    }

    pub fn find_name_key(&self, user_name: &str) -> Result<Option<EntityKey>, DomainError> {
        if !self.open.load(Ordering::Relaxed) {
            return Err(DomainError::SessionClosed);
        }

        self.performer.find_name_key(user_name)
    }

    pub fn evaluate_and_perform(
        &self,
        user_name: &str,
        text: &str,
    ) -> Result<Option<Box<dyn Reply>>> {
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

    fn get_audience_keys(&self, audience: &Audience) -> Result<Vec<EntityKey>> {
        match audience {
            Audience::Nobody => Ok(Vec::new()),
            Audience::Everybody => todo![],
            Audience::Individuals(keys) => Ok(keys.to_vec()),
            Audience::Area(area) => tools::get_occupant_keys(area),
        }
    }

    fn flush_raised<T: Notifier>(&self, notifier: &T) -> Result<()> {
        let mut pending = self.raised.borrow_mut();
        let npending = pending.len();
        if npending == 0 {
            return Ok(());
        }

        info!(%npending, "session:raising");

        for event in pending.iter() {
            let audience_keys = self.get_audience_keys(&event.audience())?;
            for key in audience_keys {
                let user = self.load_entity_by_key(&key)?.unwrap();
                debug!(%key, "observing {:?}", user);
                let observed = event.observe(&user.try_into()?)?;
                let rc: Rc<dyn Observed> = observed.into();
                notifier.notify(&key, &rc)?;
            }
        }

        pending.clear();

        Ok(())
    }

    pub fn close<T: Notifier>(&self, notifier: &T) -> Result<()> {
        self.save_entity_changes()?;

        self.flush_raised(notifier)?;

        let nentities = self.entities.size();
        let elapsed = self.opened.elapsed();
        let elapsed = format!("{:?}", elapsed);

        info!(%elapsed, %nentities, "session:closed");

        self.open.store(false, Ordering::Relaxed);

        Ok(())
    }

    fn check_for_changes(&self, l: &mut LoadedEntity) -> Result<Option<ModifiedEntity>> {
        use treediff::diff;
        use treediff::tools::ChangeType;
        use treediff::tools::Recorder;

        let _span = span!(Level::DEBUG, "flushing", key = l.key.to_string()).entered();

        let value_after = {
            let entity = l.entity.borrow();

            serde_json::to_value(&*entity)?
        };

        let value_before: serde_json::Value = if let Some(serialized) = &l.serialized {
            serialized.parse()?
        } else {
            serde_json::Value::Null
        };

        let mut d = Recorder::default();
        diff(&value_before, &value_after, &mut d);

        let modifications = d
            .calls
            .iter()
            .filter(|c| !matches!(c, ChangeType::Unchanged(_, _)))
            .count();

        if modifications > 0 {
            for each in d.calls {
                match each {
                    ChangeType::Unchanged(_, _) => {}
                    _ => debug!("modified: {:?}", each),
                }
            }

            // Serialize to string now that we know we'll use this.
            let serialized = value_after.to_string();

            // Assign new global identifier if necessary.
            let gid = match &l.gid {
                Some(gid) => gid.clone(),
                None => self.ids.get(),
            };
            l.gid = Some(gid.clone());

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
        if self.save_modified_entities()? {
            // We only do this if we actually saved any entities, that's the
            // only way this can possible change.
            self.save_modified_ids()?;

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
        // We may need a cleaner or even faster way of doing these loads.
        let world = self.entry(&WORLD_KEY)?.ok_or(DomainError::EntityNotFound)?;

        // Check to see if the global identifier has changed due to the creation
        // of a new entity.
        let previous_gid =
            identifiers::model::get_gid(&world)?.unwrap_or_else(|| EntityGid::new(0));
        let new_gid = self.ids.gid();
        if previous_gid != new_gid {
            info!(%previous_gid, %new_gid, "gid:changed");
            identifiers::model::set_gid(&world, new_gid)?;
        } else {
            info!(%previous_gid, "gid:same");
        }

        Ok(())
    }

    fn find_item_in_set(
        &self,
        haystack: &EntityRelationshipSet,
        item: &Item,
    ) -> Result<Option<Entry>> {
        match item {
            Item::Gid(gid) => self.entry_by_gid(gid),
            _ => haystack.find_item(item),
        }
    }

    fn load_entity_by_key(&self, key: &EntityKey) -> Result<Option<EntityPtr>> {
        self.entities.prepare_entity_by_key(key)
    }
}

impl Infrastructure for Session {
    fn entry_by_gid(&self, gid: &EntityGid) -> Result<Option<Entry>> {
        if let Some(e) = self.entities.prepare_entity_by_gid(gid)? {
            self.entry(&e.key())
        } else {
            Ok(None)
        }
    }

    fn entry(&self, key: &EntityKey) -> Result<Option<Entry>> {
        match self.load_entity_by_key(key)? {
            Some(entity) => Ok(Some(Entry {
                key: key.clone(),
                entity,
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

    fn ensure_entity(&self, entity_ref: &EntityRef) -> Result<EntityRef, DomainError> {
        if entity_ref.has_entity() {
            Ok(entity_ref.clone())
        } else if let Some(entity) = &self.load_entity_by_key(&entity_ref.key)? {
            Ok(entity.into())
        } else {
            Err(DomainError::EntityNotFound)
        }
    }

    fn add_entity(&self, entity: &EntityPtr) -> Result<Entry> {
        self.entities.add_entity(entity)?;

        Ok(self
            .entry(&entity.key())?
            .expect("Bug: Newly added entity has no Entry"))
    }

    fn obliterate(&self, entry: &Entry) -> Result<()> {
        let destroying = entry.entity()?;
        let mut destroying = destroying.borrow_mut();
        destroying.destroy()?;

        self.destroyed.borrow_mut().push(entry.key());

        Ok(())
    }

    fn chain(&self, living: &Entry, action: Box<dyn Action>) -> Result<Box<dyn Reply>> {
        self.performer.perform(living, action)
    }

    fn new_key(&self) -> EntityKey {
        self.keys.following()
    }

    fn new_identity(&self) -> Identity {
        self.identities.following()
    }

    fn raise(&self, event: Box<dyn DomainEvent>) -> Result<()> {
        self.raised.borrow_mut().push(event);

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
