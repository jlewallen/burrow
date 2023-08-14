use crate::library::model::*;

pub trait BeforeMovingHook {
    fn before_moving(&self, surroundings: &Surroundings, to_area: &EntityPtr) -> Result<CanMove>;
}

impl BeforeMovingHook for MovingHooks {
    fn before_moving(&self, surroundings: &Surroundings, to_area: &EntityPtr) -> Result<CanMove> {
        Ok(self
            .before_moving
            .instances
            .borrow()
            .iter()
            .map(|h| h.before_moving(surroundings, to_area))
            .collect::<Result<Vec<CanMove>>>()?
            .iter()
            .fold(CanMove::default(), |c, h| c.fold(h)))
    }
}

pub trait AfterMoveHook {
    fn after_move(&self, surroundings: &Surroundings, from_area: &EntityPtr) -> Result<()>;
}

impl AfterMoveHook for MovingHooks {
    fn after_move(&self, surroundings: &Surroundings, from_area: &EntityPtr) -> Result<()> {
        self.after_move
            .instances
            .borrow()
            .iter()
            .map(|h| h.after_move(surroundings, from_area))
            .collect::<Result<Vec<()>>>()?;

        Ok(())
    }
}

#[derive(Default)]
pub struct MovingHooks {
    pub before_moving: Hooks<Box<dyn BeforeMovingHook>>,
    pub after_move: Hooks<Box<dyn AfterMoveHook>>,
}

impl HooksSet for MovingHooks {
    fn hooks_key() -> &'static str
    where
        Self: Sized,
    {
        "moving"
    }
}

#[derive(Clone, Default)]
pub enum CanMove {
    #[default]
    Allow,
    Prevent,
}

impl HookOutcome for CanMove {
    fn fold(&self, other: &Self) -> Self {
        match (self, other) {
            (_, CanMove::Prevent) => CanMove::Prevent,
            (CanMove::Prevent, _) => CanMove::Prevent,
            (_, _) => CanMove::Allow,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Occupying {
    pub area: EntityRef,
}

impl Scope for Occupying {
    fn scope_key() -> &'static str {
        "occupying"
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SimpleRoute {
    name: String,
    to: EntityKey,
}

impl SimpleRoute {
    pub fn new(name: &str, to: EntityKey) -> Self {
        Self {
            name: name.to_owned(),
            to,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Route {
    Simple(SimpleRoute),
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Occupyable {
    pub acls: Acls,
    pub occupied: Vec<EntityRef>,
    pub occupancy: u32,
    pub routes: Option<Vec<Route>>,
}

impl Occupyable {
    pub fn stop_occupying(&mut self, item: &EntityPtr) -> Result<DomainOutcome> {
        let before = self.occupied.len();
        self.occupied.retain(|i| *i.key() != item.key());
        let after = self.occupied.len();
        if before == after {
            return Ok(DomainOutcome::Nope);
        }

        Ok(DomainOutcome::Ok)
    }

    pub fn start_occupying(&mut self, item: &EntityPtr) -> Result<DomainOutcome> {
        self.occupied.push(item.entity_ref());

        Ok(DomainOutcome::Ok)
    }
}

impl Scope for Occupyable {
    fn scope_key() -> &'static str {
        "occupyable"
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Exit {
    pub area: EntityRef,
}

impl Scope for Exit {
    fn scope_key() -> &'static str {
        "exit"
    }
}
