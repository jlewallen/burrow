use serde::Serialize;

use crate::model::EntityPtr;

#[derive(Debug, Clone, Serialize)]
pub enum Surroundings {
    Actor {
        world: EntityPtr,
        actor: EntityPtr,
        area: EntityPtr,
    },
}

impl Surroundings {
    pub fn unpack(&self) -> (EntityPtr, EntityPtr, EntityPtr) {
        match self {
            Surroundings::Actor { world, actor, area } => {
                (world.clone(), actor.clone(), area.clone())
            }
        }
    }

    pub fn world(&self) -> &EntityPtr {
        match self {
            Surroundings::Actor {
                world,
                actor: _,
                area: _,
            } => world,
        }
    }

    pub fn actor(&self) -> &EntityPtr {
        match self {
            Surroundings::Actor {
                world: _,
                actor,
                area: _,
            } => actor,
        }
    }
}
