use anyhow::Result;
use std::rc::Rc;

use super::{DevNullNotifier, Session};
use crate::{
    kernel::{ActionArgs, EntityKey, EntityPtr, Entry, SessionRef, Surroundings, WORLD_KEY},
    plugins::tools,
};

pub struct Build {
    session: SessionRef,
    entry: Option<Entry>,
    entity: EntityPtr,
}

impl Build {
    pub fn new(session: &Rc<Session>) -> Result<Self> {
        let entity = EntityPtr::new_blank()?;

        Ok(Self {
            session: session.clone(),
            entity,
            entry: None,
        })
    }

    pub fn key(&mut self, key: &EntityKey) -> Result<&mut Self> {
        assert!(self.entry.is_none());
        self.entity.set_key(key)?;

        Ok(self)
    }

    pub fn named(&mut self, name: &str) -> Result<&mut Self> {
        assert!(self.entry.is_none());
        self.entity.set_name(name)?;

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

    pub fn into_entry(&mut self) -> Result<Entry> {
        match &self.entry {
            Some(entry) => Ok(entry.clone()),
            None => Ok(self.session.add_entity(&self.entity)?),
        }
    }
}

pub struct BuildActionArgs {
    hands: Vec<QuickThing>,
    ground: Vec<QuickThing>,
    session: Rc<Session>,
}

pub enum QuickThing {
    Object(&'static str),
    Multiple(&'static str, f32),
    Place(&'static str),
    Route(&'static str, Box<QuickThing>),
    Actual(Entry),
}

impl QuickThing {
    pub fn make(&self, session: &Rc<Session>) -> Result<Entry> {
        match self {
            QuickThing::Object(name) => Ok(Build::new(session)?.named(name)?.into_entry()?),
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

impl BuildActionArgs {
    pub fn new_in_session(session: Rc<Session>) -> Result<Self> {
        Ok(Self {
            hands: Vec::new(),
            ground: Vec::new(),
            session,
        })
    }

    pub fn new() -> Result<Self> {
        let storage_factory = crate::storage::sqlite::Factory::new(":memory:")?;
        let domain = crate::domain::Domain::new(storage_factory, true);
        let session = domain.open_session()?;

        Ok(Self {
            hands: Vec::new(),
            ground: Vec::new(),
            session,
        })
    }

    pub fn build(&mut self) -> Result<Build> {
        Build::new(&self.session)
    }

    pub fn make(&mut self, q: QuickThing) -> Result<Entry> {
        q.make(&self.session)
    }

    pub fn hands(&mut self, items: Vec<QuickThing>) -> &mut Self {
        self.hands.extend(items);

        self
    }

    pub fn route(&mut self, route_name: &'static str, destination: QuickThing) -> &mut Self {
        self.ground(vec![QuickThing::Route(route_name, Box::new(destination))])
    }

    pub fn ground(&mut self, items: Vec<QuickThing>) -> &mut Self {
        self.ground.extend(items);

        self
    }

    pub fn plain(&mut self) -> &mut Self {
        self
    }

    pub fn flush(&mut self) -> Result<&mut Self> {
        self.session.flush()?;

        Ok(self)
    }

    pub fn close(&mut self) -> Result<&mut Self> {
        self.session.close(&DevNullNotifier::default())?;

        Ok(self)
    }
}

impl TryFrom<&mut BuildActionArgs> for ActionArgs {
    type Error = anyhow::Error;

    fn try_from(builder: &mut BuildActionArgs) -> Result<Self, Self::Error> {
        let world = Build::new(&builder.session)?
            .key(&WORLD_KEY)?
            .named("World")?
            .into_entry()?;

        let person = Build::new(&builder.session)?
            .named("Living")?
            .holding(
                &builder
                    .hands
                    .iter()
                    .map(|i| -> Result<_> { i.make(&builder.session) })
                    .collect::<Result<Vec<_>>>()?,
            )?
            .into_entry()?;

        let area = Build::new(&builder.session)?
            .named("Welcome Area")?
            .occupying(&vec![person.clone()])?
            .holding(
                &builder
                    .ground
                    .iter()
                    .map(|i| -> Result<_> { i.make(&builder.session) })
                    .collect::<Result<Vec<_>>>()?,
            )?
            .into_entry()?;

        builder.session.flush()?;

        let session: SessionRef = Rc::clone(&builder.session) as SessionRef;

        Ok(ActionArgs::new(
            Surroundings::Living {
                world,
                living: person,
                area,
            },
            session,
        ))
    }
}
