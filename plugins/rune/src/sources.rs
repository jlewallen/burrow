use anyhow::Result;
use glob::glob;
use plugins_core::{EntityRelationship, EntityRelationshipSet};
use std::path::PathBuf;
use tracing::*;

use kernel::prelude::{EntityKey, EntityPtr, OpenScope, Surroundings};

use crate::Behaviors;

pub static RUNE_EXTENSION: &str = "rn";

// Not super happy about Clone here, this is so we can store them mapped to
// RuneRunners and makes building that hash easier. Maybe, move to generating a
// key from this and using that.
#[derive(Clone)]
pub enum ScriptSource {
    File(PathBuf),
    System(String),
    Entity(EntityKey, String),
}

impl ScriptSource {
    pub fn source(&self) -> Result<rune::Source> {
        match self {
            ScriptSource::File(path) => Ok(rune::Source::from_path(path.as_path())?),
            ScriptSource::Entity(key, source) => Ok(rune::Source::new(key.to_string(), source)),
            ScriptSource::System(source) => Ok(rune::Source::new("system".to_string(), source)),
        }
    }
}

#[derive(Clone)]
pub struct SimpleScope {
    owner: EntityKey,
    relation: Relation,
}

#[derive(Clone)]
pub struct Script {
    pub(super) source: ScriptSource,
    pub(super) scope: Option<SimpleScope>,
}

impl Script {
    pub fn source(&self) -> Result<rune::Source> {
        self.source.source()
    }
}

pub fn load_user_sources() -> Result<Vec<Script>> {
    let mut scripts = Vec::new();
    for file in glob("user/*.rn")? {
        match file {
            Ok(path) => {
                info!("script {}", path.display());
                scripts.push(Script {
                    source: ScriptSource::File(path),
                    scope: None,
                });
            }
            Err(e) => warn!("{:?}", e),
        }
    }

    Ok(scripts)
}

#[derive(Clone)]
pub enum Relation {
    World,
    User,
    Area,
    Holding,
    Occupying,
    Ground,
    Contained,
    Wearing,
}

impl Relation {
    fn new(value: &EntityRelationship) -> Self {
        match value {
            EntityRelationship::World(_) => Self::World,
            EntityRelationship::User(_) => Self::User,
            EntityRelationship::Area(_) => Self::Area,
            EntityRelationship::Holding(_) => Self::Holding,
            EntityRelationship::Occupying(_) => Self::Occupying,
            EntityRelationship::Ground(_) => Self::Ground,
            EntityRelationship::Contained(_) => Self::Contained,
            EntityRelationship::Wearing(_) => Self::Wearing,
        }
    }
}

#[allow(dead_code)]
pub fn load_sources_from_surroundings(surroundings: &Surroundings) -> Result<Vec<Script>> {
    let mut scripts = Vec::new();
    let haystack = EntityRelationshipSet::new_from_surroundings(surroundings).expand()?;
    for nearby in haystack.iter() {
        let entity = nearby.entity()?;
        trace!(key = ?entity.key(), "check-sources");
        if let Some(script) = get_script(entity)? {
            info!("script {:?}", nearby);
            let relation = Relation::new(nearby);
            let source = ScriptSource::Entity(entity.key().clone(), script);
            scripts.push(Script {
                source,
                scope: Some(SimpleScope {
                    owner: entity.key(),
                    relation: relation,
                }),
            });
        }
    }

    Ok(scripts)
}

pub fn get_script(entity: &EntityPtr) -> Result<Option<String>> {
    let behaviors = entity.scope::<Behaviors>()?.unwrap_or_default();
    match &behaviors.langs {
        Some(langs) => match langs.get(RUNE_EXTENSION) {
            Some(script) => Ok(Some(script.clone())),
            None => Ok(None),
        },
        None => Ok(None),
    }
}
