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

use super::internal::{DomainInfrastructure, EntityMap, GlobalIds, LoadedEntity, Performer};
use super::perform::StandardPerformer;
use super::{Entry, Notifier, Sequence};
use crate::kernel::*;
use crate::plugins::identifiers;
use crate::plugins::tools;
use crate::storage::{EntityStorage, PersistedEntity};

struct ModifiedEntity(PersistedEntity);

pub struct Session {
    opened: Instant,
    open: AtomicBool,
    storage: Rc<dyn EntityStorage>,
    entity_map: Rc<EntityMap>,
    ids: Rc<GlobalIds>,
    infra: Rc<DomainInfrastructure>,
    performer: Rc<StandardPerformer>,
    raised: Rc<RefCell<Vec<Box<dyn DomainEvent>>>>,
    weak: Weak<Session>,
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
        let standard_performer = StandardPerformer::new(None);
        let performer = standard_performer.clone() as Rc<dyn Performer>;
        let raised = Rc::new(RefCell::new(Vec::new()));
        let domain_infra = DomainInfrastructure::new(
            Rc::clone(&storage),
            Rc::clone(&entity_map),
            Rc::clone(&performer),
            Arc::clone(keys),
            Arc::clone(identities),
            Rc::clone(&raised),
        );

        let infra = domain_infra.clone() as InfrastructureRef;
        standard_performer.initialize(infra.clone());

        storage.begin()?;

        if let Some(world) = infra.entry(&WORLD_KEY)? {
            if let Some(gid) = identifiers::model::get_gid(&world)? {
                ids.set(&gid);
            }
        }

        let session = Rc::new_cyclic(move |weak: &Weak<Session>| Self {
            opened,
            infra: domain_infra,
            storage,
            entity_map,
            open: AtomicBool::new(true),
            performer: standard_performer,
            ids,
            raised,
            weak: Weak::clone(weak),
        });

        session.set_session()?;

        Ok(session)
    }

    fn set_session(&self) -> Result<()> {
        let infra: Rc<dyn Infrastructure> = self
            .weak
            .upgrade()
            .ok_or_else(|| DomainError::NoInfrastructure)?;
        set_my_session(Some(&infra))?;

        Ok(())
    }

    pub fn entry(&self, key: &EntityKey) -> Result<Option<Entry>> {
        match self.infra.load_entity_by_key(key)? {
            Some(_) => Ok(Some(Entry {
                key: key.clone(),
                session: Rc::downgrade(&self.infra) as Weak<dyn Infrastructure>,
            })),
            None => Ok(None),
        }
    }

    pub fn scope<T: Scope>(&self, entry: &Entry) -> Result<Box<T>, DomainError> {
        let entity = entry.entity()?;

        info!("{:?} scope", entity);

        let entity = entity.borrow();

        entity.load_scope::<T>()
    }

    pub fn save<T: Scope>(&self, entry: &Entry, scope: &T) -> Result<()> {
        let entity = self.infra.load_entity_by_key(&entry.key)?.unwrap();
        let mut entity = entity.borrow_mut();

        entity.replace_scope::<T>(scope)
    }

    pub fn infra(&self) -> InfrastructureRef {
        self.infra.clone() as InfrastructureRef
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
                    // TODO Include thiat this failed as part of the error.
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
                let user = self.infra.load_entity_by_key(&key)?.unwrap();
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

        let nentities = self.entity_map.size();
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
        self.storage.save(&modified.0)
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
            .entity_map
            .foreach_entity_mut(|l| self.check_for_changes(l))?;
        Ok(modified.into_iter().flatten().collect::<Vec<_>>())
    }

    fn save_modified_ids(&self) -> Result<()> {
        // We may need a cleaner or even faster way of doing these loads.
        let world = self
            .infra
            .entry(&WORLD_KEY)?
            .ok_or(DomainError::EntityNotFound)?;

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
}

impl Infrastructure for Session {
    fn load_entity_by_key(&self, key: &EntityKey) -> Result<Option<EntityPtr>> {
        self.infra.load_entity_by_key(key)
    }

    fn entry_by_gid(&self, gid: &EntityGid) -> Result<Option<Entry>> {
        self.infra.entry_by_gid(gid)
    }

    fn find_item(&self, args: ActionArgs, item: &Item) -> Result<Option<Entry>> {
        self.infra.find_item(args, item)
    }

    fn entry(&self, key: &EntityKey) -> Result<Option<Entry>> {
        self.infra.entry(key)
    }

    fn ensure_entity(&self, entity_ref: &EntityRef) -> Result<EntityRef, DomainError> {
        self.infra.ensure_entity(entity_ref)
    }

    fn add_entity(&self, entity: &EntityPtr) -> Result<Entry> {
        self.infra.add_entity(entity)
    }

    fn new_key(&self) -> EntityKey {
        self.infra.new_key()
    }

    fn new_identity(&self) -> Identity {
        self.infra.new_identity()
    }

    fn chain(&self, living: &Entry, action: Box<dyn Action>) -> Result<Box<dyn Reply>> {
        self.infra.chain(living, action)
    }

    fn raise(&self, event: Box<dyn DomainEvent>) -> Result<()> {
        self.infra.raise(event)
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
