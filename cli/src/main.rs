use anyhow::Result;
use base64::Engine;
use clap::{Parser, Subcommand};
use ed25519_dalek::Keypair;
use nanoid::nanoid;
use rand::rngs::OsRng;
use std::{error::Error, path::PathBuf, sync::Arc};
use tracing::*;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use engine::{sequences::Sequence, storage::StorageFactory, Domain};
use kernel::{EntityKey, Identity, RegisteredPlugins};
use plugins_core::{
    building::BuildingPluginFactory, carrying::CarryingPluginFactory, chat::ChatPluginFactory,
    emote::EmotePluginFactory, helping::HelpingPluginFactory, looking::LookingPluginFactory,
    memory::MemoryPluginFactory, moving::MovingPluginFactory, security::SecurityPluginFactory,
    DefaultFinder,
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

    if !original.contains("hyper=") {
        original.push_str(",hyper=info");
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

pub struct Ed25519Identities {}

impl Sequence<Identity> for Ed25519Identities {
    fn following(&self) -> Identity {
        let mut csprng = OsRng {};
        let keypair: Keypair = Keypair::generate(&mut csprng);
        let public = keypair.public.to_bytes();
        let private = keypair.secret.to_bytes();
        let engine = base64::prelude::BASE64_STANDARD_NO_PAD;
        let public = engine.encode(public);
        let private = engine.encode(private);
        Identity::new(public, private)
    }
}

struct DomainBuilder {
    path: Option<String>,
    wasm: bool,
    dynlib: bool,
    rune: bool,
    rpc: bool,
}

impl Default for DomainBuilder {
    fn default() -> Self {
        Self {
            path: None,
            wasm: false,
            dynlib: true,
            rune: true,
            rpc: false,
        }
    }
}

impl DomainBuilder {
    pub fn new(path: Option<String>) -> DomainBuilder {
        Self {
            path,
            ..Default::default()
        }
    }

    pub fn storage_factory(&self) -> Result<sqlite::Factory> {
        Ok(Factory::new(
            self.path.as_ref().unwrap_or(&"world.sqlite3".to_owned()),
        )?)
    }

    pub async fn build(&self) -> Result<Domain> {
        let mut registered_plugins = RegisteredPlugins::default();
        if self.dynlib {
            registered_plugins.register(DynamicPluginFactory::default());
        }
        if self.rune {
            registered_plugins.register(RunePluginFactory::default());
        }
        if self.wasm {
            registered_plugins.register(WasmPluginFactory::new(&get_assets_path()?)?);
        }
        if self.rpc {
            registered_plugins.register(RpcPluginFactory::start().await?);
        }
        registered_plugins.register(LookingPluginFactory::default());
        registered_plugins.register(ChatPluginFactory::default());
        registered_plugins.register(EmotePluginFactory::default());
        registered_plugins.register(MovingPluginFactory::default());
        registered_plugins.register(CarryingPluginFactory::default());
        registered_plugins.register(MemoryPluginFactory::default());
        registered_plugins.register(SecurityPluginFactory::default());
        registered_plugins.register(HelpingPluginFactory::default());
        registered_plugins.register(BuildingPluginFactory::default());
        let finder = Arc::new(DefaultFinder::default());
        let storage_factory = Arc::new(self.storage_factory()?);
        storage_factory.migrate()?;
        Ok(Domain::new(
            storage_factory,
            Arc::new(registered_plugins),
            finder,
            Arc::new(NanoIds {}),
            Arc::new(Ed25519Identities {}),
        ))
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
                .with(tracing_subscriber::fmt::layer().with_thread_ids(false))
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
