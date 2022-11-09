use anyhow::Result;
use std::rc::Rc;

use super::new_infra;
use crate::{
    kernel::{ActionArgs, EntityPtr, Infrastructure, Needs},
    plugins::{
        carrying::model::Containing,
        moving::model::{Exit, Occupyable},
    },
};

pub fn get_infra() -> Result<Rc<dyn Infrastructure>> {
    new_infra()
}

pub struct Build {
    entity: EntityPtr,
}

impl Build {
    pub fn new(infra: &Rc<dyn Infrastructure>) -> Result<Self> {
        let entity = EntityPtr::new_blank();

        // TODO Would love to do this from `supply` except we only have
        // &self there instead of Rc<dyn Infrastructure>
        {
            let mut entity = entity.borrow_mut();
            entity.supply(infra)?;
        }

        infra.add_entity(&entity)?;

        Ok(Self { entity })
    }

    pub fn named(&self, name: &str) -> Result<&Self> {
        let mut entity = self.entity.borrow_mut();

        entity.set_name(name)?;

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
            occupyable.start_occupying(living.clone())?;
        }

        occupyable.save()?;

        Ok(self)
    }

    pub fn holding(&self, items: &Vec<EntityPtr>) -> Result<&Self> {
        let mut entity = self.entity.borrow_mut();
        let mut container = entity.scope_mut::<Containing>()?;

        for item in items {
            container.start_carrying(item.clone())?;
        }

        container.save()?;

        Ok(self)
    }

    pub fn into_entity(&self) -> EntityPtr {
        self.entity.clone()
    }
}

pub struct BuildActionArgs {
    infra: Rc<dyn Infrastructure>,
    hands: Vec<QuickThing>,
    ground: Vec<QuickThing>,
}

pub enum QuickThing {
    Object(String),
    Place(String),
    Route(String, Box<QuickThing>),
}

impl QuickThing {
    pub fn make(&self, infra: &Rc<dyn Infrastructure>) -> Result<EntityPtr> {
        match self {
            QuickThing::Object(name) => Ok(Build::new(infra)?.named(name)?.into_entity()),
            QuickThing::Place(name) => Ok(Build::new(infra)?.named(name)?.into_entity()),
            QuickThing::Route(name, area) => {
                let area = area.make(infra)?;

                Ok(Build::new(infra)?
                    .named(name)?
                    .leads_to(area)?
                    .into_entity())
            }
        }
    }
}

impl BuildActionArgs {
    pub fn new() -> Result<Self> {
        Ok(Self {
            infra: get_infra()?,
            hands: Vec::new(),
            ground: Vec::new(),
        })
    }

    pub fn hands(&mut self, items: Vec<QuickThing>) -> &Self {
        self.hands.extend(items);
        self
    }

    pub fn ground(&mut self, items: Vec<QuickThing>) -> &Self {
        self.ground.extend(items);
        self
    }

    pub fn plain(&mut self) -> &Self {
        self
    }
}

impl TryFrom<&BuildActionArgs> for ActionArgs {
    type Error = anyhow::Error;

    fn try_from(builder: &BuildActionArgs) -> Result<Self, Self::Error> {
        let infra = Rc::clone(&builder.infra);

        let world = Build::new(&infra)?.into_entity();

        let person = Build::new(&infra)?
            .named("Person")?
            .holding(
                &builder
                    .hands
                    .iter()
                    .map(|i| -> Result<_> { i.make(&infra) })
                    .collect::<Result<Vec<EntityPtr>>>()?,
            )?
            .into_entity();

        let area = Build::new(&infra)?
            .named("Starting Area")?
            .occupying(&vec![person.clone()])?
            .holding(
                &builder
                    .ground
                    .iter()
                    .map(|i| -> Result<_> { i.make(&infra) })
                    .collect::<Result<Vec<_>>>()?,
            )?
            .into_entity();

        Ok((world, person, area, infra))
    }
}
