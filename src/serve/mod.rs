use anyhow::{anyhow, Result};
use axum::{
    extract::Extension,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, get_service},
    Router,
};
use axum_typed_websockets::{Message, WebSocket, WebSocketUpgrade};
use clap::Args;
use futures::{sink::SinkExt, stream::StreamExt};
use serde::{Deserialize, Serialize};
use std::{
    borrow::Borrow,
    collections::HashSet,
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tokio::signal;
use tokio::sync::broadcast;
use tower_http::{
    services::ServeDir,
    trace::{DefaultMakeSpan, TraceLayer},
};
use tracing::{debug, info};

use crate::{
    domain::Domain,
    kernel::{Reply, SimpleReply},
    storage,
};

#[derive(Debug, Args)]
pub struct Command {}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
enum ServerMessage {
    Error(String),
    Welcome,
    Reply(serde_json::Value),
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
enum ClientMessage {
    Login { username: String },
    Evaluate(String),
}

#[tokio::main]
pub async fn execute_command(_cmd: &Command) -> Result<()> {
    info!("serving");

    let storage_factory = storage::sqlite::Factory::new("world.sqlite3")?;
    let domain = Domain::new(storage_factory, false);

    let assets_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");

    let user_set = Mutex::new(HashSet::new());
    let (tx, _rx) = broadcast::channel(100);

    let app_state = Arc::new(AppState {
        domain,
        user_set,
        tx,
    });

    let app = Router::new()
        .fallback(
            get_service(ServeDir::new(assets_dir).append_index_html_on_directories(true))
                .handle_error(|error: std::io::Error| async move {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Unhandled internal error: {}", error),
                    )
                }),
        )
        .route("/ws", get(ws_handler))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::default().include_headers(false)),
        )
        .layer(Extension(app_state));

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    debug!("listening on {}", addr);

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("hyper error"); // TODO How do we bubble this error up?

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

    info!("signal received, starting graceful shutdown");
}

async fn ws_handler(
    ws: WebSocketUpgrade<ServerMessage, ClientMessage>,
    Extension(state): Extension<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(stream: WebSocket<ServerMessage, ClientMessage>, state: Arc<AppState>) {
    // By splitting we can send and receive at the same time.
    let (mut sender, mut receiver) = stream.split();

    let mut session: Option<ClientSession> = None;

    while let Some(Ok(message)) = receiver.next().await {
        match message {
            Message::Item(ClientMessage::Login { username: given }) => {
                // If username that is sent by client is not taken, fill username string.
                if let Ok(started) = state.try_start_session(&given) {
                    session = Some(started)
                }

                break;
            }
            _ => {}
        }
    }

    if session.is_none() {
        let _ = sender
            .send(Message::Item(ServerMessage::Error(
                "Sorry, there's a problem with your credentials.".to_string(),
            )))
            .await;

        return;
    } else {
        let _ = sender.send(Message::Item(ServerMessage::Welcome)).await;
    }

    let session = session.unwrap();

    // Send joined message to all subscribers.
    let mut rx = state.tx.subscribe();
    // let msg = format!("{} joined.", session.username);
    // tracing::debug!("{}", msg);
    // let _ = state.tx.send(msg);

    // Pump messages to clients.
    let mut send_task = tokio::spawn(async move {
        while let Ok(server_message) = rx.recv().await {
            // In any websocket error, break loop.
            if sender.send(Message::Item(server_message)).await.is_err() {
                break;
            }
        }
    });

    // Pump messages from clients.
    let tx = state.tx.clone();
    let name = session.username.clone();
    let user_state = state.clone();

    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(Message::Item(message))) = receiver.next().await {
            match message {
                ClientMessage::Evaluate(text) => {
                    let app_state: &AppState = user_state.borrow();
                    let session = app_state
                        .domain
                        .open_session()
                        .expect("Error opening session");

                    let reply: Box<dyn Reply> = if let Some(reply) = session
                        .evaluate_and_perform(&name, text.as_str())
                        .expect("Evaluation error")
                    {
                        reply
                    } else {
                        Box::new(SimpleReply::What)
                    };

                    session.close().expect("Error closing session");

                    let _ = tx.send(ServerMessage::Reply(
                        reply.to_json().expect("Errror serializing reply JSON"),
                    ));
                }
                _ => todo!(),
            }
        }
    });

    // If any one of the tasks exit, abort the other.
    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    };

    // Send user left message.
    // let msg = format!("{} left.", session.username);
    // tracing::debug!("{}", msg);
    // let _ = state.tx.send(msg);

    state.remove_session(&session);
}
struct ClientSession {
    username: String,
}

struct AppState {
    domain: Domain,
    user_set: Mutex<HashSet<String>>,
    tx: broadcast::Sender<ServerMessage>,
}

impl AppState {
    pub fn try_start_session(&self, name: &str) -> Result<ClientSession> {
        if name.is_empty() {
            return Err(anyhow!("username cannot be blank"));
        }

        let mut user_set = self.user_set.lock().unwrap();

        if user_set.contains(name) {
            return Err(anyhow!("username already taken"));
        }

        user_set.insert(name.to_owned());

        Ok(ClientSession {
            username: name.to_string(),
        })
    }

    fn remove_session(&self, session: &ClientSession) {
        self.user_set.lock().unwrap().remove(&session.username);
    }
}
