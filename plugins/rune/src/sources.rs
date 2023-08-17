use anyhow::Result;
use glob::glob;
use plugins_core::EntityRelationshipSet;
use std::collections::HashSet;
use std::path::PathBuf;
use tracing::*;

use kernel::prelude::{EntityKey, EntityPtr, OpenScope, Surroundings};

use crate::Behaviors;

pub static RUNE_EXTENSION: &str = "rn";

// Not super happy about Clone here, this is so we can store them mapped to
// RuneRunners and makes building that hash easier. Maybe, move to generating a
// key from this and using that.
#[derive(Clone, PartialEq, Eq, Hash)]
pub enum ScriptSource {
    File(PathBuf),
    System(String),
    Entity(EntityKey, String),
}

pub fn load_user_sources() -> Result<HashSet<ScriptSource>> {
    let mut scripts = HashSet::new();
    for file in glob("user/*.rn")? {
        match file {
            Ok(path) => {
                info!("script {}", path.display());
                scripts.insert(ScriptSource::File(path));
            }
            Err(e) => warn!("{:?}", e),
        }
    }

    Ok(scripts)
}

pub fn load_sources_from_surroundings(
    surroundings: &Surroundings,
) -> Result<HashSet<ScriptSource>> {
    let mut scripts = HashSet::new();
    let haystack = EntityRelationshipSet::new_from_surroundings(surroundings).expand()?;
    for nearby in haystack
        .iter()
        .map(|r| r.entity())
        .collect::<Result<Vec<_>>>()?
    {
        trace!(key = ?nearby.key(), "check-sources");
        if let Some(script) = get_script(nearby)? {
            info!("script {:?}", nearby);
            scripts.insert(ScriptSource::Entity(nearby.key().clone(), script));
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
