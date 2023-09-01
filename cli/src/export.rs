use anyhow::Result;
use clap::Args;
use std::{fs::OpenOptions, io::Write, path::PathBuf};
use tracing::*;

use engine::storage::{PersistedEntity, StorageFactory};
use kernel::prelude::{Entity, EntityKey, JsonValue, OpenScope};
use plugins_rune::Behaviors;

use crate::DomainBuilder;

#[derive(Debug, Args, Clone)]
pub struct Command {
    #[arg(short, long, value_name = "FILE")]
    path: Option<String>,
    #[arg(short, long)]
    to: String,
}

impl Command {
    fn builder(&self) -> DomainBuilder {
        DomainBuilder::new(self.path.clone())
    }
}

#[tokio::main]
pub async fn execute_command(cmd: &Command) -> Result<()> {
    let builder = cmd.builder();
    let domain = builder.build().await?;

    let factory = builder.storage_factory()?;
    let _storage = factory.create_storage()?;

    let entities = domain.query_all()?.into_iter();

    let exporter = FileExporter {
        to: cmd.to.clone().into(),
    };

    for entity in entities {
        entity.export(&exporter)?;
    }

    Ok(())
}

struct FileExporter {
    to: PathBuf,
}

impl FileExporter {
    fn json(&self, key: &EntityKey, data: &JsonValue) -> Result<()> {
        let file_or_dir = self.to.join(key.key_to_string());

        let mut file_path = file_or_dir.clone();
        file_path.set_extension("json");

        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(file_path)?;

        file.write_all(serde_json::to_string_pretty(data)?.as_bytes())?;

        Ok(())
    }

    fn script(&self, key: &EntityKey, data: &str) -> Result<()> {
        let key_path = self.to.join(key.key_to_string());

        std::fs::create_dir_all(&key_path)?;

        let file_path = key_path.join("entry.rn");

        debug!("writing {:?}", file_path);

        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(file_path)?;

        file.write_all(data.as_bytes())?;

        Ok(())
    }
}

trait Export<E> {
    fn export(&self, exporter: &E) -> Result<()>;
}

impl Export<FileExporter> for PersistedEntity {
    fn export(&self, exporter: &FileExporter) -> Result<()> {
        info!("exporting {}", self.key);

        let key = EntityKey::new(&self.key);
        let json: JsonValue = serde_json::from_str(&self.serialized)?;

        let entity = Entity::from_value(json.clone())?;
        if let Some(behaviors) = entity.scope::<Behaviors>()? {
            if let Some(rune) = behaviors.get_rune() {
                exporter.script(&key, &rune.entry)?;
            }
        }

        exporter.json(&key, &json)?;

        Ok(())
    }
}
