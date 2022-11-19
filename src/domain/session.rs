use anyhow::Result;
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

pub struct Session {
    opened: Instant,
    open: AtomicBool,
    storage: Rc<dyn EntityStorage>,
    entity_map: Rc<EntityMap>,
    ids: Rc<GlobalIds>,
    infra: Rc<DomainInfrastructure>,
    performer: Rc<StandardPerformer>,
}

impl Session {
    pub fn new(storage: Rc<dyn EntityStorage>) -> Result<Self> {
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

    fn check_for_changes(&self, l: &LoadedEntity) -> Result<Option<PersistedEntity>> {
        use treediff::diff;
        use treediff::tools::ChangeType;
        use treediff::tools::Recorder;

        let entity = l.entity.borrow();

        let _span = span!(Level::DEBUG, "flushing", key = entity.key.to_string()).entered();

        let serialized = serde_json::to_string(&*entity)?;

        trace!("json: {:?}", serialized);

        let v1: serde_json::Value = if let Some(serialized) = &l.serialized {
            serialized.parse()?
        } else {
            serde_json::Value::Null
        };
        let v2: serde_json::Value = serialized.parse()?;
        let mut d = Recorder::default();
        diff(&v1, &v2, &mut d);

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

            let gid = match &l.gid {
                Some(gid) => gid.clone(),
                None => self.ids.get(),
            };

            Ok(Some(PersistedEntity {
                key: entity.key.to_string(),
                gid: gid.into(),
                version: l.version,
                serialized,
            }))
        } else {
            Ok(None)
        }
    }

    fn get_modified_entities(&self) -> Result<Vec<PersistedEntity>> {
        let saved = self
            .entity_map
            .foreach_entity(|l| self.check_for_changes(l))?;
        Ok(saved.into_iter().flatten().collect::<Vec<_>>())
    }

    fn should_flush_entities(&self) -> Result<bool> {
        Ok(!self
            .get_modified_entities()?
            .into_iter()
            .map(|p| self.storage.save(&p))
            .collect::<Result<Vec<_>>>()?
            .is_empty())
    }

    fn maybe_save_gid(&self) -> Result<()> {
        let world = self
            .infra
            .load_entity_by_key(&WORLD_KEY)?
            .ok_or(DomainError::EntityNotFound)?;
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

    fn save_entity_changes(&self) -> Result<()> {
        if self.should_flush_entities()? {
            self.maybe_save_gid()?;

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
    fn ensure_entity(&self, entity_ref: &LazyLoadedEntity) -> Result<LazyLoadedEntity> {
        self.infra.ensure_entity(entity_ref)
    }

    fn find_item(&self, args: ActionArgs, item: &Item) -> Result<Option<EntityPtr>> {
        self.infra.find_item(args, item)
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
    storage_factory: Box<dyn EntityStorageFactory>,
}

impl Domain {
    pub fn new(storage_factory: Box<dyn EntityStorageFactory>) -> Self {
        info!("domain-new");

        Domain { storage_factory }
    }

    pub fn open_session(&self) -> Result<Session> {
        info!("session-open");

        let storage = self.storage_factory.create_storage()?;

        Session::new(storage)
    }
}

fn should_force_rollback() -> bool {
    env::var("FORCE_ROLLBACK").is_ok()
}
