use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing::{debug, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub mod domain;
pub mod eval;
pub mod kernel;
pub mod library;
pub mod model;
pub mod plugins;
pub mod serve;
pub mod storage;

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
            let storage_factory = storage::sqlite::Factory::new("world.sqlite3");
            let domain = domain::Domain::new(storage_factory);
            let session = domain.open_session()?;

            for text in &["look", "hold rake", "drop", "hold rake", "drop rake"] {
                session.evaluate_and_perform("jlewallen", text)?;
            }

            Ok(())
        }
        None => Ok(()),
    }
}
