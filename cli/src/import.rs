use anyhow::Result;
use clap::Args;

use engine::storage::StorageFactory;

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
    let _domain = builder.build().await?;

    let factory = builder.storage_factory()?;
    let _storage = factory.create_storage()?;

    Ok(())
}
