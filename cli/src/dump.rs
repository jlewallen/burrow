use std::{
    io::{self, Write},
    str::FromStr,
};

use anyhow::Result;
use clap::Args;
use engine::storage::PersistedEntity;
use kernel::{CoreProps, Entity, EntityKey, LookupBy};

use crate::DomainBuilder;

#[derive(Debug, Args, Clone)]
pub struct Command {
    #[arg(short, long, value_name = "FILE")]
    path: Option<String>,
    #[arg(short, long)]
    lines: bool,
    #[arg(short, long)]
    key: Option<String>,
    #[arg(short, long)]
    name: Option<String>,
}

impl Command {
    fn builder(&self) -> DomainBuilder {
        DomainBuilder::new(self.path.clone())
    }
}

pub struct Filtered {
    persisted: PersistedEntity,
    entity: Option<Entity>,
}

impl From<Filtered> for PersistedEntity {
    fn from(val: Filtered) -> Self {
        val.persisted
    }
}

impl From<PersistedEntity> for Filtered {
    fn from(value: PersistedEntity) -> Self {
        Self {
            persisted: value,
            entity: None,
        }
    }
}

impl Filtered {
    fn hydrate(self) -> Result<Self> {
        Ok(Self {
            persisted: self.persisted.clone(),
            entity: Some(Entity::from_str(&self.persisted.serialized)?),
        })
    }

    fn entity(&self) -> Result<&Entity> {
        match &self.entity {
            Some(entity) => Ok(entity),
            None => panic!(),
        }
    }
}

#[tokio::main]
pub async fn execute_command(cmd: &Command) -> Result<()> {
    let builder = cmd.builder();
    let domain = builder.build().await?;

    let entities: Vec<PersistedEntity> = match &cmd.key {
        Some(key) => domain
            .query_entity(&LookupBy::Key(&EntityKey::new(key)))
            .into_iter()
            .flatten()
            .collect(),
        None => domain
            .query_all()?
            .into_iter()
            .map(Filtered::from)
            .map(|f| {
                if cmd.name.is_some() {
                    Ok(f.hydrate()?)
                } else {
                    Ok(f)
                }
            })
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .filter(|f| {
                cmd.name
                    .as_ref()
                    .and_then(|pattern| {
                        f.entity()
                            .unwrap()
                            .name()
                            .map(|name| name.contains(pattern))
                    })
                    .unwrap_or(true)
            })
            .map(|f| f.into())
            .collect(),
    };
    if cmd.lines {
        for entity in entities {
            io::stdout().write_all(entity.serialized.as_bytes())?;
        }
    } else {
        let entities: Vec<_> = entities
            .into_iter()
            .map(|p| p.to_json_value())
            .collect::<Result<Vec<_>>>()?;
        let array = serde_json::Value::Array(entities);
        io::stdout().write_all(&serde_json::to_vec(&array)?)?;
    }

    Ok(())
}
