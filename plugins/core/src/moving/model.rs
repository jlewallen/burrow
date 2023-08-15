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

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SimpleRoute {
    name: String,
    to: EntityRef,
}

impl SimpleRoute {
    pub fn new(name: &str, to: EntityRef) -> Self {
        Self {
            name: name.to_owned(),
            to,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub enum Route {
    Simple(SimpleRoute),
}

impl Route {
    fn conflicts_with(&self, other: &Route) -> bool {
        match (self, other) {
            (Route::Simple(a), Route::Simple(b)) => a.name == b.name,
        }
    }

    fn matching_name(&self, name: &str) -> bool {
        match self {
            Route::Simple(simple) => simple.name.to_lowercase().contains(&name.to_lowercase()),
        }
    }

    pub fn destination(&self) -> &EntityRef {
        match self {
            Route::Simple(simple) => &simple.to,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Occupyable {
    pub acls: Acls,
    pub occupied: Vec<EntityRef>,
    pub occupancy: u32,
    pub routes: Option<Vec<Route>>,
}

impl Occupyable {
    pub fn stop_occupying(&mut self, item: &EntityPtr) -> Result<bool, DomainError> {
        let before = self.occupied.len();
        self.occupied.retain(|i| *i.key() != item.key());
        let after = self.occupied.len();
        if before == after {
            return Ok(false);
        }

        Ok(true)
    }

    pub fn start_occupying(&mut self, item: &EntityPtr) -> Result<(), DomainError> {
        self.occupied.push(item.entity_ref());

        Ok(())
    }

    pub fn remove_route(&mut self, name: &str) -> Result<bool, DomainError> {
        if let Some(routes) = &mut self.routes {
            if let Some(found) = routes.iter().position(|r| r.matching_name(name)) {
                routes.remove(found);
                return Ok(true);
            }
        }

        Ok(false)
    }

    pub fn add_route(&mut self, route: Route) -> Result<(), DomainError> {
        let routes = self.routes.get_or_insert_with(|| Vec::new());

        if let Some(conflict) = routes.iter().position(|r| r.conflicts_with(&route)) {
            routes.remove(conflict);
        }

        routes.push(route);

        Ok(())
    }

    pub fn find_route(&self, name: &str) -> Result<Option<EntityPtr>, DomainError> {
        let Some(routes) = &self.routes else {
            return Ok(None);
        };

        for route in routes {
            if route.matching_name(name) {
                return Ok(Some(route.destination().to_entity()?));
            }
        }

        Ok(None)
    }
}

impl Scope for Occupyable {
    fn scope_key() -> &'static str {
        "occupyable"
    }
}
