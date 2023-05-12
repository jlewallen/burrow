use anyhow::Result;
use glob::glob;
use plugins_core::EntityRelationshipSet;
use std::collections::HashSet;
use std::path::PathBuf;
use tracing::*;

use kernel::{EntityKey, Entry, Surroundings};

use crate::Behaviors;

pub static RUNE_EXTENSION: &str = "rn";

#[derive(PartialEq, Eq, Hash)]
pub enum ScriptSource {
    File(PathBuf),
    Entity(EntityKey, String),
}

pub fn load_user_sources() -> Result<HashSet<ScriptSource>> {
    let mut scripts = HashSet::new();
    for entry in glob("user/*.rn")? {
        match entry {
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
        .map(|r| r.entry())
        .collect::<Result<Vec<_>>>()?
    {
        match get_script(nearby)? {
            Some(script) => {
                info!("script {:?}", nearby);
                scripts.insert(ScriptSource::Entity(nearby.key().clone(), script));
            }
            None => (),
        }
    }

    Ok(scripts)
}

pub fn get_script(entry: &Entry) -> Result<Option<String>> {
    let behaviors = entry.scope::<Behaviors>()?;
    match &behaviors.langs {
        Some(langs) => match langs.get(RUNE_EXTENSION) {
            Some(script) => Ok(Some(script.clone())),
            None => Ok(None),
        },
        None => Ok(None),
    }
}
