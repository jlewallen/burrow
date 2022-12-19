use anyhow::Result;
use clap::{Parser, Subcommand};
use std::error::Error;
use std::path::PathBuf;
use tracing::*;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::domain::DevNullNotifier;

pub mod domain;
pub mod hacking;
pub mod kernel;
pub mod plugins;
pub mod serve;
pub mod shell;
pub mod storage;
pub mod text;

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
    Hacking,
    Eval,
}

fn get_rust_log() -> String {
    let mut original =
        std::env::var("RUST_LOG").unwrap_or_else(|_| "burrow=info,tower_http=info".into());

    if !original.contains("rustyline=") {
        original.push_str(",rustyline=info");
    }

    original
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
        Some(Commands::Hacking) => Ok(hacking::execute_command()?),
        Some(Commands::Serve(cmd)) => Ok(serve::execute_command(cmd)?),
        Some(Commands::Shell(cmd)) => Ok(shell::execute_command(cmd)?),
        Some(Commands::Eval) => {
            use crate::text::Renderer;

            let renderer = Renderer::new()?;
            let storage_factory = storage::sqlite::Factory::new("world.sqlite3")?;
            let domain = domain::Domain::new(storage_factory, false);

            {
                let session = domain.open_session()?;

                for text in &[
                    "look", /*, "hold rake", "drop", "hold rake", "drop rake"*/
                ] {
                    if let Some(reply) = session.evaluate_and_perform("jlewallen", text)? {
                        // info!("reply `{}`", reply.to_json()?);
                        let text = renderer.render(reply)?;
                        println!("{}", text);
                    }
                }

                session.close(&DevNullNotifier::default())?
            }

            {
                let session = domain.open_session()?;

                for text in &["look"] {
                    if let Some(reply) = session.evaluate_and_perform("jlewallen", text)? {
                        // info!("reply `{}`", reply.to_json()?);
                        let text = renderer.render(reply)?;
                        println!("{}", text);
                    }
                }

                session.close(&DevNullNotifier::default())?
            }

            Ok(())
        }
        None => Ok(()),
    }
}
