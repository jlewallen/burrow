use anyhow::Result;
use serde::Deserialize;
use std::rc::Rc;
use tracing::info;

use super::{new_infra, Domain, Session};
use crate::{
    kernel::{ActionArgs, EntityKey, EntityPtr, Infrastructure, Needs, WORLD_KEY},
    plugins::{
        carrying::model::Containing,
        moving::model::{Exit, Occupyable},
    },
};

pub fn get_infra() -> Result<Rc<dyn Infrastructure>> {
    new_infra()
}

pub struct Build {
    infra: Rc<dyn Infrastructure>,
    entity: EntityPtr,
}

impl Build {
    pub fn new(infra: &Rc<dyn Infrastructure>) -> Result<Self> {
        let entity: EntityPtr = {
            let entity = EntityPtr::new_blank();
            entity.borrow_mut().supply(infra)?;
            entity
        };

        Ok(Self {
            infra: Rc::clone(infra),
            entity,
        })
    }

    pub fn key(&self, key: &EntityKey) -> Result<&Self> {
        let mut entity = self.entity.borrow_mut();

        entity.set_key(key)?;

        Ok(self)
    }

    pub fn named(&self, name: &str) -> Result<&Self> {
        {
            let mut entity = self.entity.borrow_mut();
            entity.set_name(name)?;
        }

        self.entity.modified()?;

        Ok(self)
    }

    pub fn leads_to(&self, area: EntityPtr) -> Result<&Self> {
        let mut entity = self.entity.borrow_mut();
        let mut exit = entity.scope_mut::<Exit>()?;

        exit.area = area.into();

        exit.save()?;

        Ok(self)
    }

    pub fn occupying(&self, living: &Vec<EntityPtr>) -> Result<&Self> {
        let mut entity = self.entity.borrow_mut();
        let mut occupyable = entity.scope_mut::<Occupyable>()?;

        for living in living {
            occupyable.start_occupying(living)?;
        }

        occupyable.save()?;

        Ok(self)
    }

    pub fn holding(&self, items: &Vec<EntityPtr>) -> Result<&Self> {
        let mut entity = self.entity.borrow_mut();
        let mut container = entity.scope_mut::<Containing>()?;

        for item in items {
            container.start_carrying(item)?;
        }

        container.save()?;

        Ok(self)
    }

    pub fn into_entity(&self) -> Result<EntityPtr> {
        self.infra.add_entity(&self.entity)?;

        Ok(self.entity.clone())
    }
}

pub struct BuildActionArgs {
    infra: Rc<dyn Infrastructure>,
    hands: Vec<QuickThing>,
    ground: Vec<QuickThing>,
    domain: Domain,
    session: Session,
}

pub enum QuickThing {
    Object(String),
    Place(String),
    Route(String, Box<QuickThing>),
    Actual(EntityPtr),
}

impl QuickThing {
    pub fn make(&self, infra: &Rc<dyn Infrastructure>) -> Result<EntityPtr> {
        match self {
            QuickThing::Object(name) => Ok(Build::new(infra)?.named(name)?.into_entity()?),
            QuickThing::Place(name) => Ok(Build::new(infra)?.named(name)?.into_entity()?),
            QuickThing::Route(name, area) => {
                let area = area.make(infra)?;

                Ok(Build::new(infra)?
                    .named(name)?
                    .leads_to(area)?
                    .into_entity()?)
            }
            QuickThing::Actual(ep) => Ok(ep.clone()),
        }
    }
}

impl BuildActionArgs {
    pub fn new() -> Result<Self> {
        let storage_factory = crate::storage::sqlite::Factory::new(":memory:")?;
        let domain = crate::domain::Domain::new(storage_factory);
        let session = domain.open_session()?;

        Ok(Self {
            infra: Rc::clone(&session.infra()),
            hands: Vec::new(),
            ground: Vec::new(),
            domain,
            session,
        })
    }

    pub fn make(&mut self, q: QuickThing) -> Result<EntityPtr> {
        q.make(&self.infra)
    }

    pub fn hands(&mut self, items: Vec<QuickThing>) -> &mut Self {
        self.hands.extend(items);
        self
    }

    pub fn route(&mut self, route_name: &str, destination: QuickThing) -> &mut Self {
        self.ground(vec![QuickThing::Route(
            route_name.to_string(),
            Box::new(destination),
        )])
    }

    pub fn ground(&mut self, items: Vec<QuickThing>) -> &mut Self {
        self.ground.extend(items);
        self
    }

    pub fn plain(&mut self) -> &mut Self {
        self
    }

    pub fn session(&self) -> Result<Session> {
        self.domain.open_session()
    }
}

impl TryFrom<&mut BuildActionArgs> for ActionArgs {
    type Error = anyhow::Error;

    fn try_from(builder: &mut BuildActionArgs) -> Result<Self, Self::Error> {
        let infra = Rc::clone(&builder.infra);

        let world = Build::new(&infra)?
            .key(&WORLD_KEY)?
            .named("World")?
            .into_entity()?;

        let person = Build::new(&infra)?
            .named("Living")?
            .holding(
                &builder
                    .hands
                    .iter()
                    .map(|i| -> Result<_> { i.make(&infra) })
                    .collect::<Result<Vec<EntityPtr>>>()?,
            )?
            .into_entity()?;

        let area = Build::new(&infra)?
            .named("Welcome Area")?
            .occupying(&vec![person.clone()])?
            .holding(
                &builder
                    .ground
                    .iter()
                    .map(|i| -> Result<_> { i.make(&infra) })
                    .collect::<Result<Vec<_>>>()?,
            )?
            .into_entity()?;

        for entity in [&world, &person, &area] {
            info!("{:?}", entity);
        }

        builder.session.flush()?;

        Ok((world, person, area, infra))
    }
}

pub struct Constructed {}

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

pub fn from_json(s: &str) -> Result<Constructed> {
    let _parsed: JsonWorld = serde_json::from_str(s)?;
    Ok(Constructed {})
}
