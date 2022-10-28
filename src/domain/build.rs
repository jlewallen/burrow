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

    pub fn holding(&self, item: &EntityPtr) -> Result<&Self> {
        let mut entity = self.entity.borrow_mut();
        let mut container = entity.scope_mut::<Containing>()?;

        container.hold(Rc::clone(item))?;
        container.save()?;

        Ok(self)
    }

    pub fn into_entity(&self) -> EntityPtr {
        Rc::clone(&self.entity)
    }

    pub fn action_args(&self) -> Result<ActionArgsBuilder> {
        ActionArgsBuilder::new()
    }
}

pub struct ActionArgsBuilder {
    infra: Rc<dyn Infrastructure>,
}

impl ActionArgsBuilder {
    pub fn new() -> Result<Self> {
        Ok(Self {
            infra: get_infra()?,
        })
    }
}

impl TryFrom<ActionArgsBuilder> for ActionArgs {
    type Error = anyhow::Error;

    fn try_from(builder: ActionArgsBuilder) -> Result<Self, Self::Error> {
        let infra = builder.infra;

        let world = Build::new(&infra)?.into_entity();
        let person = Build::new(&infra)?.into_entity();
        let area = Build::new(&infra)?.into_entity();

        Ok((world, person, area, infra))
    }
}
