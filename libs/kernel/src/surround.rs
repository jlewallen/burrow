use serde::Serialize;

use crate::model::EntityPtr;

#[derive(Debug, Clone, Serialize)]
pub enum Surroundings {
    Living {
        world: EntityPtr,
        living: EntityPtr,
        area: EntityPtr,
    },
}

impl Surroundings {
    pub fn unpack(&self) -> (EntityPtr, EntityPtr, EntityPtr) {
        match self {
            Surroundings::Living {
                world,
                living,
                area,
            } => (world.clone(), living.clone(), area.clone()),
        }
    }

    pub fn world(&self) -> &EntityPtr {
        match self {
            Surroundings::Living {
                world,
                living: _,
                area: _,
            } => world,
        }
    }

    pub fn living(&self) -> &EntityPtr {
        match self {
            Surroundings::Living {
                world: _,
                living,
                area: _,
            } => living,
        }
    }
}
