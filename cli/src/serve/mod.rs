use anyhow::Result;
use chrono::Utc;
use clap::Args;
use std::path::PathBuf;
use std::{net::SocketAddr, sync::Arc};
use tokio::signal;
use tokio::time::sleep;
use tracing::*;

use crate::DomainBuilder;

mod handlers;
mod jwt_auth;
mod route;
mod rpc;
mod state;
mod ws;

use handlers::*;
use state::*;
use ws::*;

#[derive(Debug, Args)]
pub struct Command {
    #[arg(short, long, value_name = "FILE")]
    path: Option<String>,
}

impl Command {
    fn builder(&self) -> DomainBuilder {
        DomainBuilder::new(self.path.clone())
    }
}

#[tokio::main]
pub async fn execute_command(cmd: &Command) -> Result<()> {
    info!("serving");

    let builder = cmd.builder();
    let domain = builder.build().await?;
    let app_state = Arc::new(AppState::new(domain.clone()));

    tokio::task::spawn({
        let notifier = app_state.notifier();
        let domain = domain.clone();
        async move {
            loop {
                sleep(std::time::Duration::from_secs(1)).await;
                let now = Utc::now();
                if let Err(e) = domain.tick(now, &notifier) {
                    warn!("tick failed: {:?}", e);
                }
            }
        }
    });

    let assets_dir = PathBuf::new().join("assets");

    info!("assets: {:?}", &assets_dir);

    let app = route::create_router(assets_dir, app_state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    info!("listening on {}", addr);

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("hyper error");

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    println!();

    info!("signal received, starting graceful shutdown");
}
