use anyhow::{Error, Result};

use tracing::{debug, info};

use clap::Args;

#[derive(Debug, Args)]
pub struct Command {}

pub fn execute_command(cmd: &Command) -> Result<()> {
    info!("serving");

    Ok(())
}
