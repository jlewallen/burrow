use anyhow::Result;
use std::sync::atomic::AtomicU64;
use std::time::Instant;
use std::{
    cell::RefCell,
    env,
    rc::Rc,
    sync::atomic::{AtomicBool, Ordering},
};
use tracing::{debug, event, info, span, trace, warn, Level};

use super::internal::{DomainInfrastructure, EntityMap, GlobalIds, LoadedEntity, Performer};
use crate::plugins::{identifiers, moving::model::Occupying, users::model::Usernames};
use crate::storage::{EntityStorage, EntityStorageFactory, PersistedEntity};
use crate::{kernel::*, plugins::eval};

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
            action.perform((world, user, area, infra))?
        };

        event!(Level::INFO, "done");

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

    fn evaluate_name(&self, name: &str) -> Result<(EntityPtr, EntityPtr, EntityPtr)> {
        let _span = span!(Level::DEBUG, "L").entered();

        let infra = self.infra.borrow();

        let world = infra
            .as_ref()
            .ok_or(DomainError::NoInfrastructure)?
            .load_entity_by_key(&WORLD_KEY)?
            .ok_or(DomainError::EntityNotFound)?;

        let usernames: OpenScope<Usernames> = {
            let world = world.borrow();
            world.scope::<Usernames>()?
        };

        let user_key = &usernames.users[name];

        let living = infra
            .as_ref()
            .ok_or(DomainError::NoInfrastructure)?
            .load_entity_by_key(user_key)?
            .ok_or(DomainError::EntityNotFound)?;

        self.evaluate_living(&living)
    }

    fn evaluate_living(&self, living: &EntityPtr) -> Result<(EntityPtr, EntityPtr, EntityPtr)> {
        let world = self
            .infra
            .borrow()
            .as_ref()
            .ok_or(DomainError::NoInfrastructure)?
            .load_entity_by_key(&WORLD_KEY)?
            .ok_or(DomainError::EntityNotFound)?;

        let area: EntityPtr = {
            let user = living.borrow();
            let occupying: OpenScope<Occupying> = user.scope::<Occupying>()?;
            occupying.area.into_entity()?
        };

        info!("area {}", area.borrow());

        Ok((world, living.clone(), area))
    }

    fn discover_from(&self, entities: Vec<&EntityPtr>) -> Result<Vec<EntityKey>> {
        let _span = span!(Level::DEBUG, "D").entered();
        let mut discovered: Vec<EntityKey> = vec![];
        if self.discoverying {
            for entity in &entities {
                eval::discover(&entity.borrow(), &mut discovered)?;
            }
            info!("discovered {:?}", discovered);
        }
        Ok(discovered)
    }
}

impl Performer for StandardPerformer {
    fn perform(&self, living: &EntityPtr, action: Box<dyn Action>) -> Result<Box<dyn Reply>> {
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
            action.perform((world, living, area, infra))?
        };

        event!(Level::INFO, "done");

        Ok(reply)
    }
}

struct ModifiedEntity {
    persisting: PersistedEntity,
}

pub struct Session {
    opened: Instant,
    sequence: Rc<AtomicU64>,
    open: AtomicBool,
    storage: Rc<dyn EntityStorage>,
    entity_map: Rc<EntityMap>,
    ids: Rc<GlobalIds>,
    infra: Rc<DomainInfrastructure>,
    performer: Rc<StandardPerformer>,
}

impl Session {
    pub fn new(storage: Rc<dyn EntityStorage>, sequence: Rc<AtomicU64>) -> Result<Self> {
        info!("session-new");

        let opened = Instant::now();
        let ids = GlobalIds::new();
        let entity_map = EntityMap::new(Rc::clone(&ids));
        let standard_performer = StandardPerformer::new(None);
        let performer = standard_performer.clone() as Rc<dyn Performer>;
        let domain_infra = DomainInfrastructure::new(
            Rc::clone(&storage),
            Rc::clone(&entity_map),
            Rc::clone(&performer),
        );

        let infra = domain_infra.clone() as Rc<dyn Infrastructure>;
        standard_performer.initialize(infra.clone());

        set_my_session(Some(&infra))?;

        storage.begin()?;

        if let Some(world) = infra.load_entity_by_key(&WORLD_KEY)? {
            if let Some(gid) = identifiers::model::get_gid(&world)? {
                ids.set(&gid);
            }
        }

        Ok(Self {
            opened,
            sequence,
            infra: domain_infra,
            storage,
            entity_map,
            open: AtomicBool::new(true),
            performer: standard_performer,
            ids,
        })
    }

    pub fn infra(&self) -> Rc<dyn Infrastructure> {
        self.infra.clone() as Rc<dyn Infrastructure>
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

    pub fn close(&self) -> Result<()> {
        self.save_entity_changes()?;

        self.open.store(false, Ordering::Relaxed);

        let nentities = self.entity_map.size();
        let elapsed = self.opened.elapsed();
        let elapsed = format!("{:?}", elapsed);

        info!(%elapsed, %nentities, "session:closed");

        Ok(())
    }

    pub fn take_from_sequence(&self) -> Result<u64> {
        Ok(self.sequence.fetch_add(1, Ordering::Relaxed))
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
            .load_entity_by_key(&WORLD_KEY)?
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

impl FindsItems for Session {
    fn find_item(&self, args: ActionArgs, item: &Item) -> Result<Option<EntityPtr>> {
        self.infra.find_item(args, item)
    }
}

impl Infrastructure for Session {
    fn ensure_entity(&self, entity_ref: &LazyLoadedEntity) -> Result<LazyLoadedEntity> {
        self.infra.ensure_entity(entity_ref)
    }

    fn add_entity(&self, entity: &EntityPtr) -> Result<()> {
        self.infra.add_entity(entity)
    }

    fn chain(&self, living: &EntityPtr, action: Box<dyn Action>) -> Result<Box<dyn Reply>> {
        self.infra.chain(living, action)
    }
}

impl LoadEntities for Session {
    fn load_entity_by_key(&self, key: &EntityKey) -> Result<Option<EntityPtr>> {
        self.infra.load_entity_by_key(key)
    }

    fn load_entity_by_gid(&self, gid: &EntityGID) -> Result<Option<EntityPtr>> {
        self.infra.load_entity_by_gid(gid)
    }
}

impl SessionTrait for Session {}

pub struct Domain {
    sequence: Rc<AtomicU64>,
    storage_factory: Box<dyn EntityStorageFactory>,
}

impl Domain {
    pub fn new(storage_factory: Box<dyn EntityStorageFactory>) -> Self {
        info!("domain-new");

        Domain {
            sequence: Rc::new(AtomicU64::new(0)),
            storage_factory,
        }
    }

    pub fn open_session(&self) -> Result<Session> {
        info!("session-open");

        let storage = self.storage_factory.create_storage()?;

        Session::new(storage, Rc::clone(&self.sequence))
    }
}

fn should_force_rollback() -> bool {
    env::var("FORCE_ROLLBACK").is_ok()
}
