use anyhow::Result;
use serde::Serialize;
use tracing::debug;

use crate::{location::Location, moving::model::Occupying, tools};
use kernel::prelude::{
    get_my_session, here, Audience, DomainError, EntityPtr, Finder, Found, IntoEntityPtr, Item,
    OpenScope, Surroundings,
};

/// Determines if an entity matches a user's description of that entity, given
/// no other context at all.
/// TODO Not very excited about this returning Result.
pub fn matches_description(entity: &EntityPtr, desc: &str) -> Result<bool> {
    Ok(matches_string(&entity.name()?, desc))
}

pub fn matches_string(haystack: &str, desc: &str) -> bool {
    haystack.to_lowercase().contains(&desc.to_lowercase())
}

#[derive(Debug, Clone, Serialize)]
pub enum EntityRelationship {
    World(EntityPtr),
    Actor(EntityPtr),
    Area(EntityPtr),
    Holding(EntityPtr),
    Occupying(EntityPtr),
    Ground(EntityPtr),
    Contained(EntityPtr),
    Wearing(EntityPtr),
}

impl EntityRelationship {
    pub fn entity(&self) -> Result<&EntityPtr> {
        Ok(match self {
            EntityRelationship::World(e) => e,
            EntityRelationship::Actor(e) => e,
            EntityRelationship::Area(e) => e,
            EntityRelationship::Holding(e) => e,
            EntityRelationship::Occupying(e) => e,
            EntityRelationship::Ground(e) => e,
            EntityRelationship::Contained(e) => e,
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
            Surroundings::Actor { world, actor, area } => Self {
                entities: vec![
                    EntityRelationship::World(world.clone()),
                    EntityRelationship::Area(area.clone()),
                    EntityRelationship::Actor(actor.clone()),
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
                EntityRelationship::Actor(actor) => {
                    expanded.extend(
                        tools::contained_by(actor)?
                            .into_iter()
                            .map(EntityRelationship::Holding)
                            .collect::<Vec<_>>(),
                    );
                    expanded.extend(
                        tools::worn_by(actor)?
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

    pub fn find_item(&self, item: &Item) -> Result<Option<Found>> {
        debug!("haystack {:?}", self);

        match item {
            Item::Area => {
                for entity in &self.entities {
                    if let EntityRelationship::Area(e) = entity {
                        return Ok(Some(e.clone().into()));
                    }
                }

                Ok(None)
            }
            Item::Myself => {
                for entity in &self.entities {
                    if let EntityRelationship::Actor(e) = entity {
                        return Ok(Some(e.clone().into()));
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
                                return Ok(Some(e.clone().into()));
                            }
                        }
                        _ => {}
                    }
                }

                Ok(None)
            }
            Item::Contained(contained) => self.expand()?.find_item(contained),
            Item::Held(held) => self
                .prioritize(|e| match e {
                    EntityRelationship::Holding(_) => 0,
                    _ => default_priority(e),
                })?
                .find_item(held),
            Item::Quantified(q, i) => Ok(self
                .find_item(i)?
                .map(|e| e.one())
                .map_or(Ok(None), |v| v.map(Some))?
                .map(|e| Found::Quantified(q.clone(), e))),
            _ => Ok(None),
        }
    }

    #[allow(dead_code)]
    fn filter<P>(&self, mut predicate: P) -> EntityRelationshipSet
    where
        P: FnMut(&EntityRelationship) -> bool,
    {
        let entities = self
            .entities
            .iter()
            .filter(|r| predicate(r))
            .map(|i| i.clone())
            .collect();
        Self { entities }
    }

    fn prioritize<F>(&self, mut order: F) -> Result<EntityRelationshipSet>
    where
        F: FnMut(&EntityRelationship) -> u32,
    {
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
        EntityRelationship::Actor(_) => 8,
        EntityRelationship::World(_) => 9,
    }
}

#[derive(Default)]
pub struct DefaultFinder {}

impl DefaultFinder {
    fn find_top_container(&self, entity: EntityPtr) -> Result<EntityPtr, DomainError> {
        if let Some(container) = entity.scope::<Location>()? {
            match &container.container {
                Some(container) => self.find_top_container(container.to_entity()?),
                None => Ok(entity),
            }
        } else {
            Ok(entity)
        }
    }
}

impl Finder for DefaultFinder {
    fn find_world(&self) -> Result<EntityPtr, DomainError> {
        Ok(get_my_session()?.world()?.expect("No world"))
    }

    fn find_area(&self, entity: &EntityPtr) -> Result<EntityPtr, DomainError> {
        let entity = self.find_top_container(entity.clone())?;

        if let Some(occupying) = entity.scope::<Occupying>()? {
            return Ok(occupying.area.to_entity()?);
        }

        Ok(entity)
    }

    fn find_item(
        &self,
        surroundings: &Surroundings,
        item: &Item,
    ) -> Result<Option<Found>, DomainError> {
        let haystack = EntityRelationshipSet::new_from_surroundings(surroundings).expand()?;
        Ok(haystack.find_item(item)?)
    }

    fn find_audience(
        &self,
        audience: &kernel::prelude::Audience,
    ) -> Result<Vec<kernel::prelude::EntityKey>, DomainError> {
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
                Ok(tools::get_occupant_keys(&area)?)
            }
        }
    }
}
