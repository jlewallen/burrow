use anyhow::Result;
use serde::Deserialize;
use std::{rc::Rc, sync::atomic::AtomicU64, sync::atomic::Ordering};
use tracing::info;

use super::Session;
use crate::{
    kernel::{ActionArgs, EntityKey, EntityPtr, Infrastructure, WORLD_KEY},
    plugins::{
        carrying::model::Containing,
        moving::model::{Exit, Occupyable},
    },
};

pub struct Build {
    infra: Rc<dyn Infrastructure>,
    entity: EntityPtr,
}

static COUNTER: AtomicU64 = AtomicU64::new(0);

impl Build {
    pub fn new(session: &Session) -> Result<Self> {
        let infra = session.infra();
        let entity = EntityPtr::new_blank();

        let deterministic_key =
            EntityKey::new(&format!("E-{}", COUNTER.fetch_add(1, Ordering::Relaxed)));

        {
            let mut modifying = entity.borrow_mut();
            modifying.set_key(&deterministic_key)?;
        }
        entity.modified()?;

        Ok(Self { infra, entity })
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
    hands: Vec<QuickThing>,
    ground: Vec<QuickThing>,
    session: Session,
}

pub enum QuickThing {
    Object(String),
    Place(String),
    Route(String, Box<QuickThing>),
    Actual(EntityPtr),
}

impl QuickThing {
    pub fn make(&self, session: &Session) -> Result<EntityPtr> {
        match self {
            QuickThing::Object(name) => Ok(Build::new(session)?.named(name)?.into_entity()?),
            QuickThing::Place(name) => Ok(Build::new(session)?.named(name)?.into_entity()?),
            QuickThing::Route(name, area) => {
                let area = area.make(session)?;

                Ok(Build::new(session)?
                    .named(name)?
                    .leads_to(area)?
                    .into_entity()?)
            }
            QuickThing::Actual(ep) => Ok(ep.clone()),
        }
    }
}

impl BuildActionArgs {
    pub fn new_in_session(session: Session) -> Result<Self> {
        Ok(Self {
            hands: Vec::new(),
            ground: Vec::new(),
            session,
        })
    }

    pub fn new() -> Result<Self> {
        let storage_factory = crate::storage::sqlite::Factory::new(":memory:")?;
        let domain = crate::domain::Domain::new(storage_factory);
        let session = domain.open_session()?;

        Ok(Self {
            hands: Vec::new(),
            ground: Vec::new(),
            session,
        })
    }

    pub fn make(&mut self, q: QuickThing) -> Result<EntityPtr> {
        q.make(&self.session)
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

    pub fn flush(&mut self) -> Result<&mut Self> {
        self.session.flush()?;
        Ok(self)
    }

    pub fn close(&mut self) -> Result<&mut Self> {
        self.session.close()?;
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
            .into_entity()?;

        let person = Build::new(&builder.session)?
            .named("Living")?
            .holding(
                &builder
                    .hands
                    .iter()
                    .map(|i| -> Result<_> { i.make(&builder.session) })
                    .collect::<Result<Vec<EntityPtr>>>()?,
            )?
            .into_entity()?;

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
