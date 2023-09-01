use std::{fs::OpenOptions, io::Write, path::PathBuf};

use anyhow::Result;
use clap::Args;

use engine::storage::{PersistedEntity, StorageFactory};
use kernel::prelude::{Entity, EntityKey};
use serde_json::Value as JsonValue;

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

    #[allow(unused_variables, dead_code)]
    fn script(&self, key: &EntityKey, data: &str) -> Result<()> {
        todo!()
    }
}

trait Export<E> {
    fn export(&self, exporter: &E) -> Result<()>;
}

impl Export<FileExporter> for PersistedEntity {
    fn export(&self, exporter: &FileExporter) -> Result<()> {
        let key = EntityKey::new(&self.key);
        let json: JsonValue = serde_json::from_str(&self.serialized)?;

        exporter.json(&key, &json)?;

        let _entity = Entity::from_value(json);

        Ok(())
    }
}
