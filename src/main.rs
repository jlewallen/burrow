use std::path::PathBuf;

use anyhow::Result;

use tracing::{debug, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use clap::{Parser, Subcommand};

pub mod eval;
pub mod serve;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(short, long, value_name = "FILE")]
    path: Option<PathBuf>,

    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Serve(serve::Command),
    Eval,
}

fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "rudder=debug,tower_http=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("initialized, ready");
    debug!("debug enabled");

    let cli = Cli::parse();

    match &cli.command {
        Some(Commands::Serve(cmd)) => serve::execute_command(cmd),
        Some(Commands::Eval) => {
            for text in &["look", "hold rake", "drop"] {
                let action = eval::evaluate(text)?;
                let _performed = action.perform()?;
            }

            Ok(())
        }
        None => Ok(()),
    }
}
