use crate::model::Entry;

#[derive(Debug, Clone)]
pub enum Surroundings {
    Living {
        world: Entry,
        living: Entry,
        area: Entry,
    },
}

impl Surroundings {
    pub fn unpack(&self) -> (Entry, Entry, Entry) {
        match self {
            Surroundings::Living {
                world,
                living,
                area,
            } => (world.clone(), living.clone(), area.clone()),
        }
    }

    pub fn living(&self) -> &Entry {
        match self {
            Surroundings::Living {
                world: _,
                living,
                area: _,
            } => living,
        }
    }
}
