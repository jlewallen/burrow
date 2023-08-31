use crate::library::model::*;

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

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn destination(&self) -> &EntityRef {
        &self.to
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub enum Route {
    Simple(SimpleRoute),
    Deactivated(String, Box<Route>),
}

impl Route {
    fn name(&self) -> &str {
        match self {
            Route::Simple(simple) => &simple.name,
            Route::Deactivated(_, route) => route.name(),
        }
    }

    fn conflicts_with(&self, other: &Route) -> bool {
        self.name() == other.name()
    }

    fn matching_name(&self, name: &str) -> bool {
        match self {
            Route::Simple(simple) => simple.name.to_lowercase().contains(&name.to_lowercase()),
            Route::Deactivated(_, route) => route.matching_name(name),
        }
    }

    pub fn destination(&self) -> Option<&EntityRef> {
        match self {
            Route::Simple(simple) => Some(&simple.to),
            Route::Deactivated(_, _) => None,
        }
    }

    fn activated(&self) -> Route {
        match self {
            Route::Simple(_) => self.clone(),
            Route::Deactivated(_, route) => *route.clone(),
        }
    }

    fn deactivated(&self, reason: &str) -> Route {
        match self {
            Route::Simple(_) => Route::Deactivated(reason.to_owned(), self.clone().into()),
            Route::Deactivated(_, _) => self.clone(),
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
    pub(crate) fn stop_occupying(&mut self, item: &EntityPtr) -> Result<bool, DomainError> {
        let before = self.occupied.len();
        self.occupied.retain(|i| *i.key() != item.key());
        let after = self.occupied.len();
        if before == after {
            return Ok(false);
        }

        Ok(true)
    }

    pub(crate) fn start_occupying(&mut self, item: &EntityPtr) -> Result<(), DomainError> {
        self.occupied.push(item.entity_ref());

        Ok(())
    }

    pub(crate) fn remove_route(&mut self, name: &str) -> bool {
        if let Some(routes) = &mut self.routes {
            if let Some(found) = routes.iter().position(|r| r.matching_name(name)) {
                routes.remove(found);
                return true;
            }
        }

        false
    }

    pub(crate) fn add_route(&mut self, route: Route) {
        let routes = self.routes.get_or_insert_with(|| Vec::new());

        if let Some(conflict) = routes.iter().position(|r| r.conflicts_with(&route)) {
            routes.remove(conflict);
        }

        routes.push(route);
    }

    pub(crate) fn find_route(&self, name: &str) -> Option<&Route> {
        let Some(routes) = &self.routes else {
            return None;
        };

        for route in routes {
            if route.matching_name(name) {
                return Some(route);
            }
        }

        None
    }

    pub(crate) fn activate(&mut self, name: &str) {
        let Some(routes) = &self.routes else {
            return;
        };

        self.routes = Some(
            routes
                .into_iter()
                .map(|r| {
                    if r.matching_name(name) {
                        r.activated()
                    } else {
                        r.clone()
                    }
                })
                .collect::<Vec<Route>>(),
        );
    }

    pub(crate) fn deactivate(&mut self, name: &str, reason: &str) {
        let Some(routes) = &self.routes else {
            return;
        };

        self.routes = Some(
            routes
                .into_iter()
                .map(|r| {
                    if r.matching_name(name) {
                        r.deactivated(reason)
                    } else {
                        r.clone()
                    }
                })
                .collect::<Vec<Route>>(),
        );
    }
}

impl Scope for Occupyable {
    fn scope_key() -> &'static str {
        "occupyable"
    }
}
