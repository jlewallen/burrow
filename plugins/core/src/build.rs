use anyhow::Result;
use std::{rc::Rc, sync::Arc};

use engine::{domain, prelude::*, sequences::DeterministicKeys, storage::InMemoryStorageFactory};
use kernel::{
    prelude::{
        build_entity, CoreProps, Entity, EntityKey, EntityPtr, OpenScopeRefMut, RegisteredPlugins,
        SessionRef, SetSession, Surroundings, WORLD_KEY,
    },
    session::ActiveSession,
};

use crate::{
    fashion::model::Wearable,
    helping::model::Wiki,
    moving::model::{Occupyable, Route, SimpleRoute},
    tools, DefaultFinder,
};

pub struct BuildEntityPtr {
    entity: EntityPtr,
}

impl BuildEntityPtr {
    pub fn wiki(&mut self) -> Result<&mut Self> {
        {
            let mut wiki = self.entity.scope_mut::<Wiki>()?;
            wiki.set_default("# Hello, world!");
            wiki.save()?;
        }

        Ok(self)
    }

    pub fn encyclopedia(&mut self, entity: &EntityPtr) -> Result<&mut Self> {
        self.entity.set_encyclopedia(&entity.key())?;

        Ok(self)
    }

    pub fn carryable(&mut self) -> Result<&mut Self> {
        tools::set_quantity(&self.entity, 1.0)?;

        Ok(self)
    }

    pub fn wearable(&mut self) -> Result<&mut Self> {
        self.entity.scope_mut::<Wearable>()?.save()?;

        Ok(self)
    }

    pub fn of_quantity(&mut self, quantity: f32) -> Result<&mut Self> {
        tools::set_quantity(&self.entity, quantity)?;

        Ok(self)
    }

    pub fn leads_to(&mut self, area: EntityPtr) -> Result<&mut Self> {
        tools::leads_to(&self.entity, &area)?;

        Ok(self)
    }

    pub fn routes(&mut self, routes: Vec<Route>) -> Result<&mut Self> {
        {
            let mut occupyable = self.entity.scope_mut::<Occupyable>()?;
            occupyable.routes = Some(routes);
            occupyable.save()?;
        }

        Ok(self)
    }

    pub fn occupying(&mut self, living: &Vec<EntityPtr>) -> Result<&mut Self> {
        tools::set_occupying(&self.entity, living)?;

        Ok(self)
    }

    pub fn holding(&mut self, items: &Vec<EntityPtr>) -> Result<&mut Self> {
        tools::set_container(&self.entity, items)?;

        Ok(self)
    }

    pub fn wearing(&mut self, items: &Vec<EntityPtr>) -> Result<&mut Self> {
        tools::set_wearing(&self.entity, items)?;

        Ok(self)
    }

    pub fn with_username(&mut self, name: &str, key: &EntityKey) -> Result<&mut Self> {
        self.entity.add_username_to_key(name, key)?;

        Ok(self)
    }

    pub fn into_entity(&mut self) -> Result<EntityPtr> {
        Ok(self.entity.clone())
    }
}

pub struct Build {
    session: SessionRef,
    entity: Entity,
}

impl Build {
    pub fn new(session: &Rc<Session>) -> Result<Self> {
        let entity = build_entity().with_key(session.new_key()).try_into()?;

        Self::from_entity(session, entity)
    }

    pub fn from_entity(session: &Rc<Session>, entity: Entity) -> Result<Self> {
        Ok(Self {
            session: session.clone(),
            entity,
        })
    }

    pub fn new_world(session: &Rc<Session>) -> Result<Self> {
        let entity = build_entity().with_key(WORLD_KEY.into()).try_into()?;

        Self::from_entity(session, entity)
    }

    pub fn named(&mut self, name: &str) -> Result<&mut Self> {
        {
            self.entity.set_name(name)?;
        }

        Ok(self)
    }
    pub fn save(&mut self) -> Result<BuildEntityPtr> {
        let entity = self.session.add_entity(self.entity.clone())?;
        assert!(entity.borrow().gid().is_some());
        Ok(BuildEntityPtr { entity })
    }

    pub fn into_entity(&mut self) -> Result<EntityPtr> {
        self.save()?.into_entity()
    }
}

pub enum QuickThing {
    Object(&'static str),
    Wearable(&'static str),
    Multiple(&'static str, f32),
    Place(&'static str),
    Actual(EntityPtr),
}

impl QuickThing {
    pub fn make(&self, session: &Rc<Session>) -> Result<EntityPtr> {
        match self {
            QuickThing::Object(name) => Ok(Build::new(session)?
                .named(name)?
                .save()?
                .carryable()?
                .into_entity()?),
            QuickThing::Wearable(name) => Ok(Build::new(session)?
                .named(name)?
                .save()?
                .carryable()?
                .wearable()?
                .into_entity()?),
            QuickThing::Multiple(name, quantity) => Ok(Build::new(session)?
                .named(name)?
                .save()?
                .of_quantity(*quantity)?
                .into_entity()?),
            QuickThing::Place(name) => {
                Ok(Build::new(session)?.named(name)?.save()?.into_entity()?)
            }
            QuickThing::Actual(ep) => Ok(ep.clone()),
        }
    }
}

pub enum QuickRoute {
    Simple(&'static str, EntityPtr),
}

pub struct BuildSurroundings {
    hands: Vec<QuickThing>,
    ground: Vec<QuickThing>,
    routes: Vec<QuickRoute>,
    wearing: Vec<QuickThing>,
    world: EntityPtr,
    #[allow(dead_code)] // TODO Combine with Rc<Session>?
    set: SetSession<Session>,
    session: Rc<Session>,
}

impl BuildSurroundings {
    pub fn new() -> Result<Self> {
        let keys = Arc::new(DeterministicKeys::new());
        let identities = Arc::new(DeterministicKeys::new());
        let storage_factory = Arc::new(InMemoryStorageFactory::default());
        let plugins = Arc::new(RegisteredPlugins::default());
        let finder = Arc::new(DefaultFinder::default());
        let domain = domain::Domain::new(storage_factory, plugins, finder, keys, identities);
        let session = domain.open_session()?;
        let set = session.set_session()?;

        let world = Build::new_world(&session)?
            .named("World")?
            .save()?
            .into_entity()?;

        let encyclopedia = Build::new(&session)?
            .named("Encyclopedia")?
            .save()?
            .wiki()?
            .into_entity()?;

        world.set_encyclopedia(&encyclopedia.key())?;

        Ok(Self {
            hands: Vec::new(),
            ground: Vec::new(),
            routes: Vec::new(),
            wearing: Vec::new(),
            session,
            world,
            set,
        })
    }

    pub fn new_in_session(session: Rc<Session>) -> Result<Self> {
        let set = session.set_session()?;

        let world = Build::new_world(&session)?.named("World")?.into_entity()?;

        Ok(Self {
            hands: Vec::new(),
            ground: Vec::new(),
            wearing: Vec::new(),
            routes: Vec::new(),
            session,
            world,
            set,
        })
    }

    pub fn plain(&mut self) -> &mut Self {
        self
    }

    pub fn encyclopedia(&mut self) -> Result<&mut Self> {
        Ok(self)
    }

    pub fn entity(&mut self) -> Result<Build> {
        Build::new(&self.session)
    }

    pub fn make(&mut self, q: QuickThing) -> Result<EntityPtr> {
        q.make(&self.session)
    }

    pub fn hands(&mut self, items: Vec<QuickThing>) -> &mut Self {
        self.hands.extend(items);

        self
    }

    pub fn wearing(&mut self, items: Vec<QuickThing>) -> &mut Self {
        self.wearing.extend(items);

        self
    }

    pub fn ground(&mut self, items: Vec<QuickThing>) -> &mut Self {
        self.ground.extend(items);

        self
    }

    pub fn route(&mut self, route_name: &'static str, destination: QuickThing) -> &mut Self {
        self.routes.push(QuickRoute::Simple(
            route_name,
            match destination {
                QuickThing::Actual(actual) => actual,
                _ => todo!(),
            },
        ));

        self
    }

    pub fn build(&mut self) -> Result<(SessionRef, Surroundings)> {
        let person = Build::new(&self.session)?
            .named("Living")?
            .save()?
            .wearing(
                &self
                    .wearing
                    .iter()
                    .map(|i| -> Result<_> { i.make(&self.session) })
                    .collect::<Result<Vec<_>>>()?,
            )?
            .holding(
                &self
                    .hands
                    .iter()
                    .map(|i| -> Result<_> { i.make(&self.session) })
                    .collect::<Result<Vec<_>>>()?,
            )?
            .into_entity()?;

        self.world.add_username_to_key("burrow", &person.key())?;

        let area = Build::new(&self.session)?
            .named("Welcome Area")?
            .save()?
            .occupying(&vec![person.clone()])?
            .routes(
                self.routes
                    .iter()
                    .map(|i| -> Result<_> {
                        match i {
                            QuickRoute::Simple(name, destination) => Ok(Route::Simple(
                                SimpleRoute::new(name, destination.entity_ref()),
                            )),
                        }
                    })
                    .collect::<Result<Vec<_>>>()?,
            )?
            .holding(
                &self
                    .ground
                    .iter()
                    .map(|i| -> Result<_> { i.make(&self.session) })
                    .collect::<Result<Vec<_>>>()?,
            )?
            .into_entity()?;

        self.flush()?;

        let session: SessionRef = Rc::clone(&self.session) as SessionRef;

        Ok((
            session,
            Surroundings::Living {
                world: self.world.clone(),
                living: person,
                area,
            },
        ))
    }

    pub fn flush(&mut self) -> Result<&mut Self> {
        self.session.flush(&DevNullNotifier {})?;

        Ok(self)
    }

    pub fn close(&mut self) -> Result<&mut Self> {
        self.session.close(&DevNullNotifier::default())?;

        Ok(self)
    }
}
