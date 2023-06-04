use anyhow::Result;
use clap::{Parser, Subcommand};
use std::{error::Error, path::PathBuf, sync::Arc};
use tracing::*;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use engine::{storage, Domain};
use kernel::RegisteredPlugins;
use plugins_core::{
    building::BuildingPluginFactory, carrying::CarryingPluginFactory,
    dynamic::DynamicPluginFactory, looking::LookingPluginFactory, moving::MovingPluginFactory,
    DefaultFinder,
};
use plugins_rpc::RpcPluginFactory;
use plugins_rune::RunePluginFactory;
use plugins_wasm::WasmPluginFactory;

mod eval;
mod hacking;
mod serve;
mod shell;
mod terminal;
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

enum LoggingStyle {
    Default,
    Hierarchical,
}

fn get_log_type() -> LoggingStyle {
    std::env::var("RUST_LOG_STYLE")
        .map(|v| match v.as_str() {
            "hier" | "hierarchical" => LoggingStyle::Hierarchical,
            _ => LoggingStyle::Default,
        })
        .unwrap_or(LoggingStyle::Default)
}

fn get_rust_log() -> String {
    let mut original = std::env::var("RUST_LOG").unwrap_or_else(|_| "burrow=info".into());

    if !original.contains("tower_http=") {
        original.push_str(",tower_http=info");
    }

    if !original.contains("rustyline=") {
        original.push_str(",rustyline=info");
    }

    if !original.contains("globset=") {
        original.push_str(",globset=info");
    }

    original
}

async fn make_domain() -> Result<Domain> {
    let storage_factory = storage::sqlite::Factory::new("world.sqlite3")?;
    let mut registered_plugins = RegisteredPlugins::default();
    registered_plugins.register(MovingPluginFactory::default());
    registered_plugins.register(LookingPluginFactory::default());
    registered_plugins.register(CarryingPluginFactory::default());
    registered_plugins.register(BuildingPluginFactory::default());
    registered_plugins.register(DynamicPluginFactory::default());
    registered_plugins.register(RunePluginFactory::default());
    registered_plugins.register(WasmPluginFactory::default());
    registered_plugins.register(RpcPluginFactory::start().await?);
    let finder = Arc::new(DefaultFinder::default());
    Ok(Domain::new(
        storage_factory,
        Arc::new(registered_plugins),
        finder,
        false,
    ))
}

fn main() -> Result<(), Box<dyn Error>> {
    color_backtrace::install();

    match get_log_type() {
        LoggingStyle::Default => {
            tracing_subscriber::registry()
                .with(tracing_subscriber::EnvFilter::new(get_rust_log()))
                .with(tracing_subscriber::fmt::layer().with_thread_ids(true))
                .init();
        }
        LoggingStyle::Hierarchical => {
            use tracing_tree::HierarchicalLayer;
            tracing_subscriber::registry()
                .with(tracing_subscriber::EnvFilter::new(get_rust_log()))
                .with(HierarchicalLayer::new(2))
                .init();
        }
    }

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
