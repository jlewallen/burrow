use anyhow::{anyhow, Result};
use tracing::{debug, info};

use crate::{moving::model::Occupying, tools};
use kernel::prelude::{
    get_my_session, here, Audience, DomainError, EntityPtr, Finder, IntoEntityPtr, Item, OpenScope,
    Surroundings,
};

/// Determines if an entity matches a user's description of that entity, given
/// no other context at all.
/// TODO Not very excited about this returning Result.
pub fn matches_description(entity: &EntityPtr, desc: &str) -> Result<bool> {
    if let Some(name) = entity.name()? {
        Ok(matches_string(&name, desc))
    } else {
        Ok(false)
    }
}

pub fn matches_string(haystack: &str, desc: &str) -> bool {
    haystack.to_lowercase().contains(&desc.to_lowercase())
}

#[derive(Debug, Clone)]
pub enum EntityRelationship {
    World(EntityPtr),
    User(EntityPtr),
    Area(EntityPtr),
    Holding(EntityPtr),
    Occupying(EntityPtr),
    Ground(EntityPtr),
    Contained(EntityPtr),
    Exit(String, EntityPtr),
    Wearing(EntityPtr),
}

impl EntityRelationship {
    pub fn entity(&self) -> Result<&EntityPtr> {
        Ok(match self {
            EntityRelationship::World(e) => e,
            EntityRelationship::User(e) => e,
            EntityRelationship::Area(e) => e,
            EntityRelationship::Holding(e) => e,
            EntityRelationship::Occupying(e) => e,
            EntityRelationship::Ground(e) => e,
            EntityRelationship::Contained(e) => e,
            EntityRelationship::Exit(_, e) => e,
            EntityRelationship::Wearing(e) => e,
        })
    }
}

#[derive(Debug)]
pub struct EntityRelationshipSet {
    entities: Vec<EntityRelationship>,
}

impl EntityRelationshipSet {
    pub fn iter(&self) -> std::slice::Iter<'_, EntityRelationship> {
        self.entities.iter()
    }

    pub fn new_from_surroundings(surroundings: &Surroundings) -> Self {
        match surroundings {
            Surroundings::Living {
                world,
                living,
                area,
            } => Self {
                entities: vec![
                    EntityRelationship::World(world.clone()),
                    EntityRelationship::Area(area.clone()),
                    EntityRelationship::User(living.clone()),
                ],
            },
        }
    }

    // It's important to notice that calling expand will recursively discover
    // more and more candidates.
    pub fn expand(&self) -> Result<Self> {
        let mut expanded = self.entities.clone();

        for entity in &self.entities {
            match entity {
                EntityRelationship::User(user) => {
                    expanded.extend(
                        tools::contained_by(user)?
                            .into_iter()
                            .map(EntityRelationship::Holding)
                            .collect::<Vec<_>>(),
                    );
                    expanded.extend(
                        tools::worn_by(user)?
                            .unwrap_or(Vec::default())
                            .into_iter()
                            .map(EntityRelationship::Wearing)
                            .collect::<Vec<_>>(),
                    );
                }
                EntityRelationship::Area(area) => {
                    expanded.extend(
                        tools::contained_by(area)?
                            .into_iter()
                            .map(EntityRelationship::Ground)
                            .collect::<Vec<_>>(),
                    );
                    expanded.extend(
                        tools::occupied_by(area)?
                            .into_iter()
                            .map(EntityRelationship::Occupying)
                            .collect::<Vec<_>>(),
                    );
                }
                EntityRelationship::Holding(holding) => expanded.extend(
                    tools::contained_by(holding)?
                        .into_iter()
                        .map(EntityRelationship::Contained)
                        .collect::<Vec<_>>(),
                ),
                _ => {}
            }
        }

        Ok(Self { entities: expanded })
    }

    // Why not just do this in expand?
    pub fn routes(&self) -> Result<Self> {
        use crate::moving::model::Exit;

        let mut expanded = self.entities.clone();

        for entity in &self.entities {
            if let EntityRelationship::Ground(item) = entity {
                if let Some(exit) = item.scope::<Exit>()? {
                    expanded.push(EntityRelationship::Exit(
                        item.name()?
                            .ok_or_else(|| anyhow!("Route name is required"))?,
                        exit.area.to_entity()?,
                    ));
                }
            }
        }

        Ok(Self { entities: expanded })
    }

    pub fn find_item(&self, item: &Item) -> Result<Option<EntityPtr>> {
        debug!("haystack {:?}", self);

        match item {
            Item::Area => {
                for entity in &self.entities {
                    if let EntityRelationship::Area(e) = entity {
                        return Ok(Some(e.clone()));
                    }
                }

                Ok(None)
            }
            Item::Myself => {
                for entity in &self.entities {
                    if let EntityRelationship::User(e) = entity {
                        return Ok(Some(e.clone()));
                    }
                }

                Ok(None)
            }
            Item::Named(name) => {
                for entity in &self.entities {
                    match entity {
                        EntityRelationship::Contained(e)
                        | EntityRelationship::Ground(e)
                        | EntityRelationship::Holding(e)
                        | EntityRelationship::Occupying(e)
                        | EntityRelationship::Wearing(e) => {
                            if matches_description(e, name)? {
                                return Ok(Some(e.clone()));
                            }
                        }
                        _ => {}
                    }
                }

                Ok(None)
            }
            Item::Route(name) => {
                let haystack = self.routes()?;

                debug!("route:haystack {:?}", haystack);

                for entity in &haystack.entities {
                    if let EntityRelationship::Exit(route_name, area) = entity {
                        if matches_string(route_name, name) {
                            info!("found: {:?} -> {:?}", route_name, area);
                            return Ok(Some(area.clone()));
                        }
                    }
                }

                Ok(None)
            }
            Item::Contained(contained) => self.expand()?.find_item(contained),
            Item::Held(held) => self
                .prioritize(&|e| match e {
                    EntityRelationship::Holding(_) => 0,
                    _ => default_priority(e),
                })?
                .find_item(held),
            _ => Ok(None),
        }
    }

    fn prioritize(
        &self,
        order: &dyn Fn(&EntityRelationship) -> u32,
    ) -> Result<EntityRelationshipSet> {
        let mut entities = self.entities.clone();
        entities.sort_by_key(|a| order(a));
        Ok(Self { entities })
    }
}

fn default_priority(e: &EntityRelationship) -> u32 {
    match e {
        EntityRelationship::Area(_) => 1,
        EntityRelationship::Ground(_) => 2,
        EntityRelationship::Holding(_) => 3,
        EntityRelationship::Contained(_) => 4,
        EntityRelationship::Occupying(_) => 5,
        EntityRelationship::Wearing(_) => 6,
        EntityRelationship::Exit(_, _) => 7,
        EntityRelationship::User(_) => 8,
        EntityRelationship::World(_) => 9,
    }
}

#[derive(Default)]
pub struct DefaultFinder {}

impl Finder for DefaultFinder {
    fn find_world(&self) -> anyhow::Result<EntityPtr> {
        Ok(get_my_session()?.world()?.expect("No world"))
    }

    fn find_location(&self, entity: &EntityPtr) -> Result<EntityPtr> {
        let occupying = entity.scope::<Occupying>()?.unwrap();
        Ok(occupying.area.to_entity()?)
    }

    fn find_item(&self, surroundings: &Surroundings, item: &Item) -> Result<Option<EntityPtr>> {
        let haystack = EntityRelationshipSet::new_from_surroundings(surroundings).expand()?;
        haystack.find_item(item)
    }

    fn find_audience(
        &self,
        audience: &kernel::prelude::Audience,
    ) -> Result<Vec<kernel::prelude::EntityKey>> {
        match audience {
            Audience::Nobody => Ok(Vec::new()),
            Audience::Everybody => todo![],
            Audience::Individuals(keys) => Ok(keys.to_vec()),
            Audience::Area(area) => {
                // If you find yourself here in the future, consider doing this
                // lookup when the event is raised rather than in here.
                let session = get_my_session()?;
                let area = session
                    .entity(&kernel::prelude::LookupBy::Key(area))?
                    .ok_or(DomainError::EntityNotFound(here!().into()))?;
                tools::get_occupant_keys(&area)
            }
        }
    }
}
