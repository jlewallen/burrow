use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::{error::Error, sync::Arc};
use tracing::*;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use engine::{storage, Domain};
use kernel::RegisteredPlugins;
use plugins_core::{
    building::BuildingPluginFactory, carrying::CarryingPluginFactory,
    looking::LookingPluginFactory, moving::MovingPluginFactory, DefaultFinder,
};
use plugins_rune::RunePluginFactory;

mod eval;
mod hacking;
mod serve;
mod shell;
mod text;

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
    Shell(shell::Command),
    Eval(eval::Command),
    Hacking,
}

fn get_rust_log() -> String {
    let mut original =
        std::env::var("RUST_LOG").unwrap_or_else(|_| "burrow=info,tower_http=info".into());

    if !original.contains("rustyline=") {
        original.push_str(",rustyline=info");
    }

    original
}

fn make_domain() -> Result<Domain> {
    let storage_factory = storage::sqlite::Factory::new("world.sqlite3")?;
    let mut registered_plugins = RegisteredPlugins::default();
    registered_plugins.register::<MovingPluginFactory>();
    registered_plugins.register::<LookingPluginFactory>();
    registered_plugins.register::<CarryingPluginFactory>();
    registered_plugins.register::<BuildingPluginFactory>();
    registered_plugins.register::<RunePluginFactory>();

    let finder = Arc::new(DefaultFinder {});
    Ok(Domain::new(
        storage_factory,
        Arc::new(registered_plugins),
        finder,
        false,
    ))
}

fn main() -> Result<(), Box<dyn Error>> {
    color_backtrace::install();

    let create_tracing_subscriber = || {
        // use tracing_tree::HierarchicalLayer;
        // HierarchicalLayer::new(2)
        tracing_subscriber::fmt::layer()
    };

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(get_rust_log()))
        .with(create_tracing_subscriber())
        .init();

    info!("initialized, ready");

    let cli = Cli::parse();

    match &cli.command {
        Some(Commands::Serve(cmd)) => Ok(serve::execute_command(cmd)?),
        Some(Commands::Shell(cmd)) => Ok(shell::execute_command(cmd)?),
        Some(Commands::Eval(cmd)) => Ok(eval::execute_command(cmd)?),
        Some(Commands::Hacking) => Ok(hacking::execute_command()?),
        None => Ok(()),
    }
}
