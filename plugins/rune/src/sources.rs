use anyhow::Result;
use glob::glob;
use std::path::PathBuf;
use tracing::*;

use kernel::prelude::{EntityKey, EntityPtr, JsonValue, OpenScope, Surroundings};
use plugins_core::{EntityRelationship, EntityRelationshipSet};

use crate::{Behaviors, LogEntry};

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

    pub fn describe(&self) -> String {
        match self {
            ScriptSource::File(path) => format!("File({:?})", path.to_str()),
            ScriptSource::System(_) => format!("System()"),
            ScriptSource::Entity(key, _) => format!("Entity({})", key),
        }
    }
}

#[derive(Clone, Debug, rune::Any)]
pub struct Owner {
    key: EntityKey,
    relation: Relation,
}

impl Owner {
    pub fn new(key: EntityKey, relation: Relation) -> Self {
        Self { key, relation }
    }

    #[inline]
    pub fn string_debug(&self, s: &mut String) -> std::fmt::Result {
        use std::fmt::Write;
        write!(s, "{:?}", self)
    }

    pub fn key(&self) -> String {
        self.key.key_to_string().to_owned()
    }

    pub fn relation(&self) -> Relation {
        self.relation.clone()
    }
}

#[derive(Clone)]
pub struct Script {
    pub(super) source: ScriptSource,
    pub(super) owner: Option<Owner>,
    pub(super) state: Option<JsonValue>,
}

impl Script {
    pub fn source(&self) -> Result<rune::Source> {
        self.source.source()
    }

    pub fn describe_source(&self) -> String {
        self.source.describe()
    }
}

pub fn load_library_sources() -> Result<Vec<Script>> {
    load_directory_sources("user/lib/*.rn")
}

pub fn load_user_sources() -> Result<Vec<Script>> {
    load_directory_sources("user/*.rn")
}

pub fn load_directory_sources(path: &str) -> Result<Vec<Script>> {
    let mut scripts = Vec::new();
    for file in glob(path)? {
        match file {
            Ok(path) => {
                info!("script {}", path.display());
                scripts.push(Script {
                    source: ScriptSource::File(path),
                    owner: None,
                    state: None,
                });
            }
            Err(e) => warn!("{:?}", e),
        }
    }

    Ok(scripts)
}

#[derive(Clone, Debug, rune::Any)]
pub enum Relation {
    Target,
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

pub fn load_sources_from_entity(entity: &EntityPtr, relation: Relation) -> Result<Option<Script>> {
    trace!(key = ?entity.key(), "check-sources");
    if let Some(script) = get_script(entity)? {
        info!("script {:?}", entity);
        let source = ScriptSource::Entity(entity.key().clone(), script.entry);
        Ok(Some(Script {
            source,
            owner: Some(Owner {
                key: entity.key(),
                relation,
            }),
            state: script.state,
        }))
    } else {
        Ok(None)
    }
}

pub fn load_sources_from_surroundings(surroundings: &Surroundings) -> Result<Vec<Script>> {
    let mut scripts = Vec::new();
    let haystack = EntityRelationshipSet::new_from_surroundings(surroundings).expand()?;
    for nearby in haystack.iter() {
        let entity = nearby.entity()?;
        let relation = Relation::new(nearby);
        if let Some(script) = load_sources_from_entity(entity, relation)? {
            scripts.push(script);
        }
    }

    Ok(scripts)
}

pub struct EntryAndState {
    entry: String,
    state: Option<JsonValue>,
}

impl EntryAndState {
    pub(crate) fn entry(&self) -> &str {
        &self.entry
    }
}

pub fn get_script(entity: &EntityPtr) -> Result<Option<EntryAndState>> {
    let behaviors = entity.scope::<Behaviors>()?.unwrap_or_default();
    match &behaviors.langs {
        Some(langs) => match langs.get(RUNE_EXTENSION) {
            Some(script) => Ok(Some(EntryAndState {
                entry: script.entry.clone(),
                state: script.state.clone(),
            })),
            None => Ok(None),
        },
        None => Ok(None),
    }
}

pub fn get_logs(entity: &EntityPtr) -> Result<Option<Vec<LogEntry>>> {
    let behaviors = entity.scope::<Behaviors>()?.unwrap_or_default();
    match &behaviors.langs {
        Some(langs) => match langs.get(RUNE_EXTENSION) {
            Some(script) => Ok(Some(script.logs.clone())),
            None => Ok(None),
        },
        None => Ok(None),
    }
}
