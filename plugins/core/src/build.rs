use anyhow::Result;
use std::{rc::Rc, sync::Arc};

use engine::{domain, prelude::*, sequences::DeterministicKeys, storage::InMemoryStorageFactory};
use kernel::{
    prelude::{
        build_entity, CoreProps, Entity, EntityKey, Entry, RegisteredPlugins, SessionRef,
        SetSession, Surroundings, WORLD_KEY,
    },
    session::ActiveSession,
};

use crate::{fashion::model::Wearable, helping::model::Wiki, tools, DefaultFinder};

pub struct Build {
    session: SessionRef,
    entry: Option<Entry>,
    entity: Option<Entity>,
}

impl Build {
    pub fn new(session: &Rc<Session>) -> Result<Self> {
        let entity = build_entity().with_key(session.new_key()).try_into()?;

        Self::from_entity(session, entity)
    }

    pub fn from_entity(session: &Rc<Session>, entity: Entity) -> Result<Self> {
        Ok(Self {
            session: session.clone(),
            entity: Some(entity),
            entry: None,
        })
    }

    pub fn new_world(session: &Rc<Session>) -> Result<Self> {
        let entity = build_entity().with_key(WORLD_KEY.into()).try_into()?;

        Self::from_entity(session, entity)
    }

    pub fn named(&mut self, name: &str) -> Result<&mut Self> {
        {
            assert!(self.entry.is_none());
            self.entity.as_mut().unwrap().set_name(name)?;
        }

        Ok(self)
    }

    pub fn wiki(&mut self) -> Result<&mut Self> {
        let entry = self.into_entry()?;
        let mut wiki = entry.scope_mut::<Wiki>()?;
        wiki.set_default("# Hello, world!");
        wiki.save()?;

        Ok(self)
    }

    pub fn encyclopedia(&mut self, entry: &Entry) -> Result<&mut Self> {
        self.into_entry()?.set_encyclopedia(entry.key())?;

        Ok(self)
    }

    pub fn carryable(&mut self) -> Result<&mut Self> {
        tools::set_quantity(&self.into_entry()?, 1.0)?;

        Ok(self)
    }

    pub fn wearable(&mut self) -> Result<&mut Self> {
        let entry = self.into_entry()?;
        entry.scope_mut::<Wearable>()?.save()?;

        Ok(self)
    }

    pub fn of_quantity(&mut self, quantity: f32) -> Result<&mut Self> {
        tools::set_quantity(&self.into_entry()?, quantity)?;

        Ok(self)
    }

    pub fn leads_to(&mut self, area: Entry) -> Result<&mut Self> {
        tools::leads_to(&self.into_entry()?, &area)?;

        Ok(self)
    }

    pub fn occupying(&mut self, living: &Vec<Entry>) -> Result<&mut Self> {
        tools::set_occupying(&self.into_entry()?, living)?;

        Ok(self)
    }

    pub fn holding(&mut self, items: &Vec<Entry>) -> Result<&mut Self> {
        tools::set_container(&self.into_entry()?, items)?;

        Ok(self)
    }

    pub fn wearing(&mut self, items: &Vec<Entry>) -> Result<&mut Self> {
        tools::set_wearing(&self.into_entry()?, items)?;

        Ok(self)
    }

    pub fn with_username(&mut self, name: &str, key: &EntityKey) -> Result<&mut Self> {
        let entry = self.into_entry()?;
        entry.add_username_to_key(name, key)?;

        Ok(self)
    }

    pub fn into_entry(&mut self) -> Result<Entry> {
        match &self.entry {
            Some(entry) => Ok(entry.clone()),
            None => {
                let entry = self.session.add_entity(self.entity.take().unwrap())?;
                assert!(entry.entity().borrow().gid().is_some());
                self.entry = Some(entry.clone());
                Ok(entry)
            }
        }
    }
}

pub enum QuickThing {
    Object(&'static str),
    Wearable(&'static str),
    Multiple(&'static str, f32),
    Place(&'static str),
    Route(&'static str, Box<QuickThing>),
    Actual(Entry),
}

impl QuickThing {
    pub fn make(&self, session: &Rc<Session>) -> Result<Entry> {
        match self {
            QuickThing::Object(name) => Ok(Build::new(session)?
                .named(name)?
                .carryable()?
                .into_entry()?),
            QuickThing::Wearable(name) => Ok(Build::new(session)?
                .named(name)?
                .carryable()?
                .wearable()?
                .into_entry()?),
            QuickThing::Multiple(name, quantity) => Ok(Build::new(session)?
                .named(name)?
                .of_quantity(*quantity)?
                .into_entry()?),
            QuickThing::Place(name) => Ok(Build::new(session)?.named(name)?.into_entry()?),
            QuickThing::Route(name, area) => {
                let area = area.make(session)?;

                Ok(Build::new(session)?
                    .named(name)?
                    .leads_to(area)?
                    .into_entry()?)
            }
            QuickThing::Actual(ep) => Ok(ep.clone()),
        }
    }
}

pub struct BuildSurroundings {
    hands: Vec<QuickThing>,
    ground: Vec<QuickThing>,
    wearing: Vec<QuickThing>,
    world: Entry,
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

        // TODO One problem at a time.
        let world = Build::new_world(&session)?.named("World")?.into_entry()?;

        let encyclopedia = Build::new(&session)?
            .named("Encyclopedia")?
            .wiki()?
            .into_entry()?;

        world.set_encyclopedia(&encyclopedia.key())?;

        Ok(Self {
            hands: Vec::new(),
            ground: Vec::new(),
            wearing: Vec::new(),
            session,
            world,
            set,
        })
    }

    pub fn new_in_session(session: Rc<Session>) -> Result<Self> {
        let set = session.set_session()?;

        // TODO One problem at a time.
        let world = Build::new_world(&session)?.named("World")?.into_entry()?;

        Ok(Self {
            hands: Vec::new(),
            ground: Vec::new(),
            wearing: Vec::new(),
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

    pub fn make(&mut self, q: QuickThing) -> Result<Entry> {
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
        self.ground(vec![QuickThing::Route(route_name, Box::new(destination))])
    }

    pub fn build(&mut self) -> Result<(SessionRef, Surroundings)> {
        let person = Build::new(&self.session)?
            .named("Living")?
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
            .into_entry()?;

        self.world.add_username_to_key("burrow", person.key())?;

        let area = Build::new(&self.session)?
            .named("Welcome Area")?
            .occupying(&vec![person.clone()])?
            .holding(
                &self
                    .ground
                    .iter()
                    .map(|i| -> Result<_> { i.make(&self.session) })
                    .collect::<Result<Vec<_>>>()?,
            )?
            .into_entry()?;

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
