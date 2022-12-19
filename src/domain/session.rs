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
use tracing::{debug, event, info, span, trace, warn, Level};

use super::internal::{DomainInfrastructure, EntityMap, GlobalIds, LoadedEntity, Performer};
use super::Entry;
use crate::plugins::tools;
use crate::plugins::{identifiers, moving::model::Occupying, users::model::Usernames};
use crate::storage::{EntityStorage, PersistedEntity};
use crate::{kernel::*, plugins::eval};

pub trait KeySequence: Send + Sync {
    fn new_key(&self) -> EntityKey;
}

pub trait IdentityFactory: Send + Sync {
    fn new_identity(&self) -> Identity;
}

pub struct StandardPerformer {
    infra: RefCell<Option<Rc<dyn Infrastructure>>>,
    discoverying: bool,
}

impl StandardPerformer {
    pub fn initialize(&self, infra: Rc<dyn Infrastructure>) {
        *self.infra.borrow_mut() = Some(infra);
    }

    pub fn new(infra: Option<Rc<dyn Infrastructure>>) -> Rc<Self> {
        Rc::new(StandardPerformer {
            infra: RefCell::new(infra),
            discoverying: false,
        })
    }

    pub fn perform_via_name(&self, name: &str, action: Box<dyn Action>) -> Result<Box<dyn Reply>> {
        info!("performing {:?}", action);

        let (world, user, area) = self.evaluate_name(name)?;

        self.discover_from(vec![&user, &area])?;

        let reply = {
            let _span = span!(Level::INFO, "A").entered();
            let infra = self
                .infra
                .borrow()
                .as_ref()
                .ok_or(DomainError::NoInfrastructure)?
                .clone();
            action.perform((world.try_into()?, user.try_into()?, area.try_into()?, infra))?
        };

        Ok(reply)
    }

    pub fn evaluate_and_perform(&self, name: &str, text: &str) -> Result<Option<Box<dyn Reply>>> {
        let _doing_span = span!(Level::INFO, "session-do", user = name).entered();

        debug!("'{}'", text);

        if let Some(action) = eval::evaluate(text)? {
            Ok(Some(self.perform_via_name(name, action)?))
        } else {
            Ok(None)
        }
    }

    pub fn find_name_key(&self, name: &str) -> Result<Option<EntityKey>, DomainError> {
        match self.evaluate_name(name) {
            Ok((_world, user, _area)) => Ok(Some(user.key())),
            Err(DomainError::EntityNotFound) => Ok(None),
            Err(err) => Err(err),
        }
    }

    fn evaluate_name(&self, name: &str) -> Result<(Entry, Entry, Entry), DomainError> {
        let _span = span!(Level::DEBUG, "L").entered();

        let infra = self.infra.borrow();

        let world = infra
            .as_ref()
            .ok_or(DomainError::NoInfrastructure)?
            .entry(&WORLD_KEY)?
            .ok_or(DomainError::EntityNotFound)?;

        let usernames = world.scope::<Usernames>()?;

        let user_key = &usernames.users[name];

        let living = infra
            .as_ref()
            .ok_or(DomainError::NoInfrastructure)?
            .load_entity_by_key(user_key)?
            .ok_or(DomainError::EntityNotFound)?;

        self.evaluate_living(&living.try_into()?)
    }

    fn evaluate_living(&self, living: &Entry) -> Result<(Entry, Entry, Entry), DomainError> {
        let world = self
            .infra
            .borrow()
            .as_ref()
            .ok_or(DomainError::NoInfrastructure)?
            .load_entity_by_key(&WORLD_KEY)?
            .ok_or(DomainError::EntityNotFound)?;

        let area: Entry = {
            let occupying = living.scope::<Occupying>()?;
            occupying.area.into_entry()?
        };

        info!("area {:?}", &area);

        Ok((world.try_into()?, living.clone(), area))
    }

    fn discover_from(&self, entities: Vec<&Entry>) -> Result<Vec<EntityKey>> {
        let _span = span!(Level::DEBUG, "D").entered();
        let mut discovered: Vec<EntityKey> = vec![];
        if self.discoverying {
            for entity in &entities {
                eval::discover(&entity, &mut discovered)?;
            }
            info!("discovered {:?}", discovered);
        }
        Ok(discovered)
    }
}

impl Performer for StandardPerformer {
    fn perform(&self, living: &Entry, action: Box<dyn Action>) -> Result<Box<dyn Reply>> {
        info!("performing {:?}", action);

        let (world, living, area) = self.evaluate_living(living)?;

        self.discover_from(vec![&living, &area])?;

        let reply = {
            let _span = span!(Level::INFO, "A").entered();
            let infra = self
                .infra
                .borrow()
                .as_ref()
                .ok_or(DomainError::NoInfrastructure)?
                .clone();
            action.perform((
                world.try_into()?,
                living.try_into()?,
                area.try_into()?,
                infra,
            ))?
        };

        event!(Level::INFO, "done");

        Ok(reply)
    }
}

struct ModifiedEntity {
    persisting: PersistedEntity,
}

pub trait Notifier {
    fn notify(&self, audience: &EntityKey, observed: &Rc<dyn Observed>) -> Result<()>;
}

pub struct DevNullNotifier {}

impl DevNullNotifier {
    pub fn new() -> Self {
        Self {}
    }
}

impl Notifier for DevNullNotifier {
    fn notify(&self, _audience: &EntityKey, _observed: &Rc<dyn Observed>) -> Result<()> {
        Ok(())
    }
}

pub struct Session {
    opened: Instant,
    open: AtomicBool,
    storage: Rc<dyn EntityStorage>,
    entity_map: Rc<EntityMap>,
    ids: Rc<GlobalIds>,
    infra: Rc<DomainInfrastructure>,
    performer: Rc<StandardPerformer>,
    raised: Rc<RefCell<Vec<Box<dyn DomainEvent>>>>,
    _weak: Weak<Session>,
}

impl Session {
    pub fn new(
        storage: Rc<dyn EntityStorage>,
        keys: &Arc<dyn KeySequence>,
        identities: &Arc<dyn IdentityFactory>,
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

        let infra = domain_infra.clone() as Rc<dyn Infrastructure>;
        standard_performer.initialize(infra.clone());

        set_my_session(Some(&infra))?;

        storage.begin()?;

        if let Some(world) = infra.entry(&WORLD_KEY)? {
            if let Some(gid) = identifiers::model::get_gid(&world)? {
                ids.set(&gid);
            }
        }

        Ok(Rc::new_cyclic(move |weak| Self {
            opened,
            infra: domain_infra,
            storage,
            entity_map,
            open: AtomicBool::new(true),
            performer: standard_performer,
            ids,
            raised: raised,
            _weak: Weak::clone(weak),
        }))
    }

    pub fn entry(&self, key: &EntityKey) -> Result<Option<Entry>> {
        match self.load_entity_by_key(key)? {
            Some(_) => Ok(Some(Entry {
                key: key.clone(),
                session: Rc::downgrade(&self.infra) as Weak<dyn Infrastructure>,
            })),
            None => Ok(None),
        }
    }

    pub fn scope<T: Scope>(&self, entry: &Entry) -> Result<Box<T>, DomainError> {
        let entity = match self.load_entity_by_key(&entry.key)? {
            None => panic!("How did you get an Entry for an unknown Entity?"),
            Some(entity) => entity,
        };

        info!("{:?} scope", entity);

        let entity = entity.borrow();

        entity.load_scope::<T>()
    }

    pub fn save<T: Scope>(&self, entry: &Entry, scope: &Box<T>) -> Result<()> {
        let entity = self.load_entity_by_key(&entry.key)?.unwrap();
        let mut entity = entity.borrow_mut();

        entity.replace_scope::<T>(scope)
    }

    pub fn infra(&self) -> Rc<dyn Infrastructure> {
        self.infra.clone() as Rc<dyn Infrastructure>
    }

    pub fn find_name_key(&self, user_name: &str) -> Result<Option<EntityKey>, DomainError> {
        if !self.open.load(Ordering::Relaxed) {
            return Err(DomainError::SessionClosed.into());
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
                    panic!("error rolling back");
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

        info!(%npending ,"session:raising");

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

            Ok(Some(ModifiedEntity {
                persisting: PersistedEntity {
                    key: l.key.to_string(),
                    gid: gid.into(),
                    version: version_being_saved,
                    serialized,
                },
            }))
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
        self.storage.save(&modified.persisting)
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
            identifiers::model::get_gid(&world)?.unwrap_or_else(|| EntityGID::new(0));
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

impl Infrastructure for Session {
    fn load_entity_by_key(&self, key: &EntityKey) -> Result<Option<EntityPtr>> {
        self.infra.load_entity_by_key(key)
    }

    fn load_entity_by_gid(&self, gid: &EntityGID) -> Result<Option<EntityPtr>> {
        self.infra.load_entity_by_gid(gid)
    }

    fn find_item(&self, args: ActionArgs, item: &Item) -> Result<Option<Entry>> {
        self.infra.find_item(args, item)
    }

    fn entry(&self, key: &EntityKey) -> Result<Option<Entry>> {
        self.infra.entry(key)
    }

    fn ensure_entity(&self, entity_ref: &LazyLoadedEntity) -> Result<LazyLoadedEntity> {
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

fn should_force_rollback() -> bool {
    env::var("FORCE_ROLLBACK").is_ok()
}
