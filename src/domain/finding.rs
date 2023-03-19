use anyhow::{anyhow, Result};
use tracing::{debug, info};

use crate::{
    kernel::{Entry, Item, Surroundings},
    plugins::tools,
};

/// Determines if an entity matches a user's description of that entity, given
/// no other context at all.
/// TODO Not very excited about this returning Result.
pub fn matches_description(entity: &Entry, desc: &str) -> Result<bool> {
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
    World(Entry),
    User(Entry),
    Area(Entry),
    Holding(Entry),
    Ground(Entry),
    /// Item is nearby, inside something else. Considering renaming this and
    /// others to better indicate how far removed they are. For example,
    /// containers in the area vs containers that are being held.
    Contained(Entry),
    Exit(String, Entry),
}

#[derive(Debug)]
pub struct EntityRelationshipSet {
    entities: Vec<EntityRelationship>,
}

impl EntityRelationshipSet {
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
                EntityRelationship::User(user) => expanded.extend(
                    tools::contained_by(user)?
                        .into_iter()
                        .map(EntityRelationship::Holding)
                        .collect::<Vec<_>>(),
                ),
                EntityRelationship::Area(area) => expanded.extend(
                    tools::contained_by(area)?
                        .into_iter()
                        .map(EntityRelationship::Ground)
                        .collect::<Vec<_>>(),
                ),
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
        use crate::plugins::moving::model::Exit;

        let mut expanded = self.entities.clone();

        for entity in &self.entities {
            if let EntityRelationship::Ground(item) = entity {
                if let Some(exit) = item.maybe_scope::<Exit>()? {
                    expanded.push(EntityRelationship::Exit(
                        item.name()?
                            .ok_or_else(|| anyhow!("Route name is required"))?,
                        exit.area.into_entry()?,
                    ));
                }
            }
        }

        Ok(Self { entities: expanded })
    }

    pub fn find_item(&self, item: &Item) -> Result<Option<Entry>> {
        match item {
            Item::Named(name) => {
                debug!("item:haystack {:?}", self);

                // https://github.com/ferrous-systems/elements-of-rust#tuple-structs-and-enum-tuple-variants-as-functions
                for entity in &self.entities {
                    match entity {
                        EntityRelationship::Contained(e) => {
                            if matches_description(e, name)? {
                                return Ok(Some(e.clone()));
                            }
                        }
                        EntityRelationship::Ground(e) => {
                            if matches_description(e, name)? {
                                return Ok(Some(e.clone()));
                            }
                        }
                        EntityRelationship::Holding(e) => {
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
                    match entity {
                        EntityRelationship::Exit(route_name, area) => {
                            if matches_string(route_name, name) {
                                info!("found: {:?} -> {:?}", route_name, area);
                                return Ok(Some(area.clone()));
                            }
                        }
                        _ => {}
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
            Item::Gid(_) => Ok(None),
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
        EntityRelationship::Exit(_, _) => 5,
        EntityRelationship::User(_) => 6,
        EntityRelationship::World(_) => 7,
    }
}
