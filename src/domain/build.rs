use anyhow::Result;
use serde::Deserialize;
use std::rc::Rc;
use tracing::*;

use super::{DevNullNotifier, Entry, Session};
use crate::{
    kernel::{ActionArgs, EntityKey, EntityPtr, Infrastructure, WORLD_KEY},
    plugins::tools,
};

pub struct Build {
    infra: Rc<dyn Infrastructure>,
    entry: Option<Entry>,
    entity: EntityPtr,
}

impl Build {
    pub fn new(session: &Session) -> Result<Self> {
        let infra = session.infra();
        let entity = EntityPtr::new_blank();

        Ok(Self {
            infra,
            entity,
            entry: None,
        })
    }

    fn entry(&mut self) -> Result<Entry> {
        let entry = match &self.entry {
            Some(entry) => entry.clone(),
            None => {
                self.infra.add_entity(&self.entity)?;
                self.infra
                    .entry(&self.entity.key())?
                    .expect("Missing newly added entity")
            }
        };
        Ok(entry)
    }

    pub fn key(&mut self, key: &EntityKey) -> Result<&mut Self> {
        self.entity.set_key(key)?;

        Ok(self)
    }

    pub fn named(&mut self, name: &str) -> Result<&mut Self> {
        self.entity.set_name(name)?;

        Ok(self)
    }

    pub fn of_quantity(&mut self, quantity: f32) -> Result<&mut Self> {
        tools::set_quantity(&self.entry()?, quantity)?;

        Ok(self)
    }

    pub fn leads_to(&mut self, area: Entry) -> Result<&mut Self> {
        tools::leads_to(&self.entry()?, &area)?;

        Ok(self)
    }

    pub fn occupying(&mut self, living: &Vec<Entry>) -> Result<&mut Self> {
        tools::set_occupying(&self.entry()?, living)?;

        Ok(self)
    }

    pub fn holding(&mut self, items: &Vec<Entry>) -> Result<&mut Self> {
        tools::set_container(&self.entry()?, items)?;

        Ok(self)
    }

    pub fn into_entry(&mut self) -> Result<Entry> {
        Ok(self.entry()?)
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
    pub fn make(&self, session: &Session) -> Result<Entry> {
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
        self.session.close(&DevNullNotifier::new())?;
        Ok(self)
    }
}

impl TryFrom<&mut BuildActionArgs> for ActionArgs {
    type Error = anyhow::Error;

    fn try_from(builder: &mut BuildActionArgs) -> Result<Self, Self::Error> {
        let infra = builder.session.infra();

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

        for entity in [&world, &person, &area] {
            trace!("{:?}", entity);
        }

        builder.session.flush()?;

        Ok((world, person, area, infra))
    }
}
struct Constructed {}

impl Constructed {}

#[derive(Deserialize)]
struct JsonWorld {
    _ground: Vec<JsonItem>,
}

#[derive(Deserialize)]
struct JsonItem {
    _name: String,
}

#[derive(Deserialize)]
struct JsonPlace {
    _name: String,
}

#[allow(dead_code)]
fn from_json(s: &str) -> Result<Constructed> {
    let _parsed: JsonWorld = serde_json::from_str(s)?;
    Ok(Constructed {})
}
