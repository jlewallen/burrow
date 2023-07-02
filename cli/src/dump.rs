use std::io::{self, Write};

use anyhow::Result;
use clap::Args;

use crate::{make_domain, PluginConfiguration};

#[derive(Debug, Args, Clone)]
pub struct Command {
    #[arg(short, long)]
    lines: bool,
}

impl Command {
    fn plugin_configuration(&self) -> PluginConfiguration {
        PluginConfiguration {
            wasm: false,
            dynlib: false,
            rune: false,
            rpc: false,
        }
    }
}

#[tokio::main]
pub async fn execute_command(cmd: &Command) -> Result<()> {
    let domain = make_domain(cmd.plugin_configuration()).await?;

    let entities = domain.query_all()?;
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
