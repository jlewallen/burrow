use crate::eval;
use crate::kernel::*;
use crate::plugins::carrying::model::Containing;
use crate::plugins::moving::model::Occupying;
use crate::plugins::users::model::Usernames;
use crate::storage::{EntityStorage, EntityStorageFactory};
use anyhow::Result;
use elsa::FrozenMap;
use std::{fmt::Debug, rc::Rc};
use tracing::{debug, info, span, Level};

#[derive(Debug)]
pub struct Session {
    entities: ProvidedEntities,
}

impl Session {
    pub fn new(
        storage: Box<dyn EntityStorage>,
        infrastructure_factory: Option<Box<dyn InfrastructureFactory>>,
    ) -> Self {
        info!("session-new");

        Self {
            entities: ProvidedEntities {
                entities: Entities::new(storage, infrastructure_factory),
            },
        }
    }

    pub fn evaluate_and_perform(&self, user_name: &str, text: &str) -> Result<Box<dyn Reply>> {
        let _doing_span = span!(Level::INFO, "session-do", user = user_name).entered();

        debug!("'{}'", text);

        let action = eval::evaluate(text)?;

        info!("performing {:?}", action);

        let world = self.entities.load_entity_by_key(&WORLD_KEY)?;

        let usernames: Box<Usernames> = world.scope::<Usernames>()?;

        let user_key = &usernames.users[user_name];

        let user = self.entities.load_entity_by_key(user_key)?;

        let occupying: Box<Occupying> = user.scope::<Occupying>()?;

        let area: Box<Entity> = occupying.area.try_into()?;

        info!(%user_name, "area {}", area);

        if true {
            let _test_span = span!(Level::INFO, "test").entered();

            let containing = area.scope::<Containing>()?;
            for here in containing.holding {
                info!("here {:?}", here.key())
            }

            let carrying = user.scope::<Containing>()?;
            for here in carrying.holding {
                info!("here {:?}", here.key())
            }

            let mut discovered_keys: Vec<EntityKey> = vec![];
            eval::discover(user, &mut discovered_keys)?;
            eval::discover(area.as_ref(), &mut discovered_keys)?;
            info!(%user_name, "discovered {:?}", discovered_keys);
        }

        let reply = action.perform((&world, &user, &area))?;

        info!(%user_name, "done {:?}", reply);

        Ok(reply)
    }

    pub fn hydrate_user_session(&self) -> Result<()> {
        Ok(())
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        info!("session-drop");
    }
}

struct Entities {
    storage: Box<dyn EntityStorage>,
    entities: FrozenMap<EntityKey, Box<Entity>>,
    infrastructure_factory: Option<Box<dyn InfrastructureFactory>>,
}

impl Debug for Entities {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Entities").finish()
    }
}

impl Entities {
    pub fn new(
        storage: Box<dyn EntityStorage>,
        infrastructure_structure: Option<Box<dyn InfrastructureFactory>>,
    ) -> Rc<Self> {
        debug!("entities-new");

        Rc::new(Self {
            storage: storage,
            entities: FrozenMap::new(),
            infrastructure_factory: infrastructure_structure,
        })
    }
}

impl PrepareEntityByKey for Entities {
    fn prepare_entity_by_key<T: Fn(&mut Entity) -> Result<()>>(
        &self,
        key: &EntityKey,
        prepare: T,
    ) -> Result<&Entity> {
        if let Some(e) = self.entities.get(key) {
            debug!(%key, "existing");
            return Ok(e);
        }

        let _loading_span = span!(Level::INFO, "entity", key = key).entered();

        info!("loading");

        let persisted = self.storage.load(key)?;

        let mut loaded: Entity = serde_json::from_str(&persisted.serialized)?;
        debug!("parsed");

        if let Some(factory) = &self.infrastructure_factory {
            debug!("new-infrastructure");
            loaded.set_infra(factory.new_infrastructure()?);
        }

        let _ = prepare(&mut loaded)?;

        let inserted = self.entities.insert(key.clone(), Box::new(loaded));

        Ok(inserted)
    }
}

pub struct Domain {
    storage_factory: Box<dyn EntityStorageFactory>,
}

impl Domain {
    pub fn new(storage_factory: Box<dyn EntityStorageFactory>) -> Self {
        info!("domain-new");

        Domain {
            storage_factory: storage_factory,
        }
    }

    pub fn open_session(&self) -> Result<Session> {
        info!("session-open");

        // TODO Consider using factory in Domain.
        let storage = self.storage_factory.create_storage()?;

        let session = Session::new(storage, None);

        // TODO get user
        // TODO get Area
        // TODO discover
        // TODO hydrate

        Ok(session)
    }
}

#[derive(Debug, Clone)]
struct ProvidedEntities {
    entities: Rc<Entities>,
}

impl LoadEntityByKey for ProvidedEntities {
    fn load_entity_by_key(&self, key: &EntityKey) -> Result<&Entity> {
        self.entities.prepare_entity_by_key(key, |e| {
            info!("prepare");
            e.set_infra(Rc::new(Infrastructure::new(self.clone())));
            Ok(())
        })
    }
}

#[derive(Debug)]
pub struct Infrastructure {
    entities: ProvidedEntities,
}

impl Infrastructure {
    fn new(entities: ProvidedEntities) -> Self {
        Self { entities: entities }
    }
}

impl DomainInfrastructure for Infrastructure {
    fn ensure_entity(&self, entity_ref: &DynamicEntityRef) -> Result<DynamicEntityRef> {
        match entity_ref {
            DynamicEntityRef::RefOnly {
                py_object: _,
                py_ref: _,
                key,
                klass: _,
                name: _,
            } => Ok(DynamicEntityRef::Entity(Box::new(
                self.entities.load_entity_by_key(&key)?.clone(), // TODO Meh
            ))),
            DynamicEntityRef::Entity(_) => Ok(entity_ref.clone()),
        }
    }
}
