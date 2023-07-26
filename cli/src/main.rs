use anyhow::Result;
use clap::{Parser, Subcommand};
use nanoid::nanoid;
use std::{error::Error, path::PathBuf, sync::Arc};
use tracing::*;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use engine::{sequences::Sequence, storage::EntityStorageFactory, Domain};
use kernel::{EntityKey, Identity, RegisteredPlugins};
use plugins_core::{
    building::BuildingPluginFactory, carrying::CarryingPluginFactory,
    looking::LookingPluginFactory, moving::MovingPluginFactory, DefaultFinder,
};
use plugins_dynlib::DynamicPluginFactory;
use plugins_rpc::RpcPluginFactory;
use plugins_rune::RunePluginFactory;
use plugins_wasm::WasmPluginFactory;
use sqlite::Factory;

mod dump;
mod eval;
mod hacking;
mod migrate;
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
    Dump(dump::Command),
    Migrate(migrate::Command),
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

    if !original.contains("cranelift_codegen=") {
        original.push_str(",cranelift_codegen=info");
    }

    original
}

struct NanoIds {}

impl Sequence<EntityKey> for NanoIds {
    fn following(&self) -> EntityKey {
        EntityKey::from_string(nanoid!())
    }
}

impl Sequence<Identity> for NanoIds {
    fn following(&self) -> Identity {
        Identity::default()
    }
}

struct PluginConfiguration {
    wasm: bool,
    dynlib: bool,
    rune: bool,
    rpc: bool,
}

impl Default for PluginConfiguration {
    fn default() -> Self {
        Self {
            wasm: false,
            dynlib: true,
            rune: false,
            rpc: false,
        }
    }
}

fn get_assets_path() -> Result<PathBuf> {
    let mut cwd = std::env::current_dir()?;
    loop {
        if cwd.join(".git").exists() {
            break;
        }

        cwd = match cwd.parent() {
            Some(cwd) => cwd.to_path_buf(),
            None => {
                return Err(anyhow::anyhow!("Error locating assets path"));
            }
        };
    }

    Ok(cwd.join("plugins/wasm/assets"))
}

async fn make_domain(plugins: PluginConfiguration) -> Result<Domain> {
    let mut registered_plugins = RegisteredPlugins::default();
    if plugins.dynlib {
        registered_plugins.register(DynamicPluginFactory::default());
    }
    if plugins.rune {
        registered_plugins.register(RunePluginFactory::default());
    }
    if plugins.wasm {
        registered_plugins.register(WasmPluginFactory::new(&get_assets_path()?)?);
    }
    if plugins.rpc {
        registered_plugins.register(RpcPluginFactory::start().await?);
    }
    registered_plugins.register(LookingPluginFactory::default());
    registered_plugins.register(MovingPluginFactory::default());
    registered_plugins.register(CarryingPluginFactory::default());
    registered_plugins.register(BuildingPluginFactory::default());
    let finder = Arc::new(DefaultFinder::default());
    let storage_factory = Arc::new(Factory::new("world.sqlite3")?);
    storage_factory.migrate()?;
    Ok(Domain::new(
        storage_factory,
        Arc::new(registered_plugins),
        finder,
        Arc::new(NanoIds {}),
        Arc::new(NanoIds {}),
    ))
}

struct RandomKeys {}

impl Sequence<EntityKey> for RandomKeys {
    fn following(&self) -> EntityKey {
        EntityKey::from_string(nanoid!())
    }
}

impl Sequence<Identity> for RandomKeys {
    fn following(&self) -> Identity {
        Identity::default()
    }
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
        Some(Commands::Dump(cmd)) => Ok(dump::execute_command(cmd)?),
        Some(Commands::Migrate(cmd)) => Ok(migrate::execute_command(cmd)?),
        Some(Commands::Hacking) => Ok(hacking::execute_command()?),
        None => Ok(()),
    }
}
