use std::{collections::HashMap, rc::Rc};

use anyhow::Result;
use clap::Args;

use engine::storage::{Storage, StorageFactory};
use kernel::prelude::{Entity, EntityKey, LookupBy, OpenScopeMut};
use plugins_rune::{Behaviors, RUNE_EXTENSION};
use tracing::*;

use crate::DomainBuilder;

#[derive(Debug, Args, Clone)]
pub struct Command {
    #[arg(short, long, value_name = "FILE")]
    path: Option<String>,
    #[arg(long)]
    from: String,
}

impl Command {
    fn builder(&self) -> DomainBuilder {
        DomainBuilder::new(self.path.clone())
    }
}

#[tokio::main]
pub async fn execute_command(cmd: &Command) -> Result<()> {
    let builder = cmd.builder();

    let factory = builder.storage_factory()?;
    let storage = factory.create_storage()?;

    let mut importer = Importer::new(storage);

    importer.begin()?;

    for entry in walkdir::WalkDir::new(&cmd.from).sort_by_key(|a| a.file_type().is_dir()) {
        let entry = entry?;
        let path = entry.path();

        if !entry.file_type().is_dir() {
            match path.extension() {
                Some(e) => {
                    if e == "json" {
                        let key = EntityKey::new(path.file_stem().unwrap().to_str().unwrap());
                        info!(key = %key, "json");

                        let data = std::fs::read_to_string(path)?;
                        let value: serde_json::Value = serde_json::from_str(&data)?;
                        importer.json(key, value)?;
                    }
                    if e == "rn" {
                        let key = EntityKey::new(
                            path.parent()
                                .unwrap()
                                .file_name()
                                .unwrap()
                                .to_str()
                                .unwrap(),
                        );
                        info!(key = %key, "script");

                        let data = std::fs::read_to_string(path)?;
                        importer.script(key, data)?;
                    }
                }
                None => warn!(path = ?path, "ignoring"),
            }
        }
    }

    importer.commit()?;

    Ok(())
}

struct Importer {
    storage: Rc<dyn Storage>,
}

impl Importer {
    fn new(storage: Rc<dyn Storage>) -> Self {
        Self { storage }
    }

    fn begin(&self) -> Result<()> {
        self.storage.begin()
    }

    fn json(&mut self, key: EntityKey, data: serde_json::Value) -> Result<()> {
        let loaded = self.storage.load(&LookupBy::Key(&key))?;

        let saving = match loaded {
            Some(mut loaded) => {
                loaded.serialized = data.to_string();
                loaded.version += 1;

                loaded
            }
            None => {
                todo!("assign gid to new entities");
                /*
                    PersistedEntity {
                    key: key.to_string(),
                    gid: todo!("assign gid to new entities"),
                    version: 1,
                    serialized: data.to_string(),
                }
                */
            }
        };

        self.storage.save(&saving)?;

        Ok(())
    }

    fn script(&mut self, key: EntityKey, script: String) -> Result<()> {
        let loaded = self.storage.load(&LookupBy::Key(&key))?;

        let saving = match loaded {
            Some(mut loaded) => {
                let json: serde_json::Value = serde_json::from_str(&loaded.serialized)?;
                let mut entity = Entity::from_value(json)?;
                let mut behaviors = entity.scope_mut::<Behaviors>()?;
                let langs = behaviors.langs.get_or_insert_with(HashMap::new);
                let ours = langs.entry(RUNE_EXTENSION.to_owned()).or_default();
                ours.entry = script;
                behaviors.save(&mut entity)?;

                loaded.serialized = entity.to_json_value()?.to_string();
                loaded.version += 1;

                loaded
            }
            None => panic!("script for unknown entity"),
        };

        self.storage.save(&saving)?;

        Ok(())
    }

    fn commit(&self) -> Result<()> {
        self.storage.commit()
    }
}
