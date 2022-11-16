use anyhow::Result;
use clap::{Parser, Subcommand};
use std::error::Error;
use std::path::PathBuf;
use tracing::{debug, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub mod domain;
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
    Eval,
}

fn get_rust_log() -> String {
    let mut original =
        std::env::var("RUST_LOG").unwrap_or_else(|_| "rudder=info,tower_http=debug".into());

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

    debug!("debug enabled");

    let cli = Cli::parse();

    match &cli.command {
        Some(Commands::Serve(cmd)) => Ok(serve::execute_command(cmd)?),
        Some(Commands::Shell(cmd)) => Ok(shell::execute_command(cmd)?),
        Some(Commands::Eval) => {
            use crate::text::Renderer;

            let renderer = Renderer::new()?;
            let storage_factory = storage::sqlite::Factory::new("world.sqlite3")?;
            let domain = domain::Domain::new(storage_factory);

            {
                let session = domain.open_session()?;

                for text in &[
                    "look", /*, "hold rake", "drop", "hold rake", "drop rake"*/
                ] {
                    if let Some(reply) = session.evaluate_and_perform("jlewallen", text)? {
                        info!("reply `{}`", reply.to_json()?);

                        let text = renderer.render(reply)?;
                        println!("{}", text);
                    }
                }

                session.close()?
            }

            {
                let session = domain.open_session()?;

                for text in &["look"] {
                    if let Some(reply) = session.evaluate_and_perform("jlewallen", text)? {
                        info!("reply `{}`", reply.to_json()?);

                        let text = renderer.render(reply)?;
                        println!("{}", text);
                    }
                }

                session.close()?
            }

            Ok(())
        }
        None => Ok(()),
    }
}
