use anyhow::Result;
use std::rc::Rc;

use super::new_infra;
use crate::{
    kernel::{ActionArgs, Entity, EntityPtr, Infrastructure, Needs},
    plugins::carrying::model::Containing,
};

pub fn get_infra() -> Result<Rc<dyn Infrastructure>> {
    new_infra()
}

pub struct Build {
    entity: EntityPtr,
}

impl Build {
    pub fn new(infra: &Rc<dyn Infrastructure>) -> Result<Self> {
        let entity = Entity::new();

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

    pub fn holding(&self, items: &Vec<EntityPtr>) -> Result<&Self> {
        let mut entity = self.entity.borrow_mut();
        let mut container = entity.scope_mut::<Containing>()?;

        for item in items {
            container.hold(Rc::clone(item))?;
        }

        container.save()?;

        Ok(self)
    }

    pub fn into_entity(&self) -> EntityPtr {
        Rc::clone(&self.entity)
    }
}

pub struct BuildActionArgs {
    infra: Rc<dyn Infrastructure>,
    hands: Vec<QuickThing>,
    ground: Vec<QuickThing>,
}

pub enum QuickThing {
    Object(String),
}

impl QuickThing {
    pub fn make(&self, infra: &Rc<dyn Infrastructure>) -> Result<EntityPtr> {
        match &*self {
            QuickThing::Object(name) => Ok(Build::new(infra)?.named(&name)?.into_entity()),
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
}

impl TryFrom<&BuildActionArgs> for ActionArgs {
    type Error = anyhow::Error;

    fn try_from(builder: &BuildActionArgs) -> Result<Self, Self::Error> {
        let infra = Rc::clone(&builder.infra);

        let world = Build::new(&infra)?.into_entity();

        let person = Build::new(&infra)?
            .holding(
                &builder
                    .hands
                    .iter()
                    .map(|i| -> Result<_> { i.make(&infra) })
                    .collect::<Result<Vec<EntityPtr>>>()?,
            )?
            .into_entity();

        let area = Build::new(&infra)?
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
