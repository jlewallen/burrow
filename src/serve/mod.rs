use anyhow::Result;
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
use std::{borrow::Borrow, net::SocketAddr, path::PathBuf, sync::Arc};
use tokio::signal;
use tokio::sync::broadcast;
use tower_http::{
    services::ServeDir,
    trace::{DefaultMakeSpan, TraceLayer},
};
use tracing::{debug, info, warn};

use crate::{
    domain::{DevNullNotifier, Domain, Notifier},
    kernel::{EntityKey, Reply, SimpleReply},
    storage,
};

#[derive(Debug, Args)]
pub struct Command {}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
enum ServerMessage {
    Error(String),
    Welcome {},
    Reply(serde_json::Value),
    Notify(String, serde_json::Value),
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
enum ClientMessage {
    Login { username: String },
    Evaluate(String),
}

struct ClientSession {
    name: String,
    key: EntityKey,
}

struct AppState {
    domain: Domain,
    tx: broadcast::Sender<ServerMessage>,
}

impl AppState {
    pub fn try_start_session(&self, name: &str, key: &EntityKey) -> Result<ClientSession> {
        Ok(ClientSession {
            name: name.to_string(),
            key: key.clone(),
        })
    }

    fn find_user_key(&self, name: &str) -> Result<Option<EntityKey>> {
        let session = self.domain.open_session().expect("Error opening session");

        let maybe_key = session.find_name_key(name)?;

        session.close(&DevNullNotifier::new())?;

        Ok(maybe_key)
    }

    fn remove_session(&self, _session: &ClientSession) {}
}

impl Notifier for AppState {
    fn notify(
        &self,
        audience: crate::kernel::EntityKey,
        observed: Box<dyn replies::Observed>,
    ) -> Result<()> {
        debug!("notify {:?} -> {:?}", audience, observed);

        let serialized = observed.to_json()?;
        let outgoing = ServerMessage::Notify(audience.into(), serialized);
        self.tx.send(outgoing)?;

        Ok(())
    }
}

#[tokio::main]
pub async fn execute_command(_cmd: &Command) -> Result<()> {
    info!("serving");

    let storage_factory = storage::sqlite::Factory::new("world.sqlite3")?;
    let domain = Domain::new(storage_factory, false);

    let assets_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");

    let (tx, _rx) = broadcast::channel(100);

    let app_state = Arc::new(AppState { domain, tx });

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

async fn ws_handler(
    ws: WebSocketUpgrade<ServerMessage, ClientMessage>,
    Extension(state): Extension<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(stream: WebSocket<ServerMessage, ClientMessage>, state: Arc<AppState>) {
    let (mut sender, mut receiver) = stream.split();

    let mut session: Option<ClientSession> = None;

    while let Some(Ok(m)) = receiver.next().await {
        match m {
            Message::Item(ClientMessage::Login { username: given }) => {
                session = match state.find_user_key(&given) {
                    Ok(Some(key)) => Some(
                        state
                            .try_start_session(&given, &key)
                            .expect("Error starting session"),
                    ),
                    Err(err) => {
                        warn!("find-user-key: {:?}", err);

                        None
                    }
                    _ => None,
                };

                break;
            }
            m => {
                info!("unexpected: {:?}", m);
            }
        }
    }

    if session.is_none() {
        info!("bad credentials");

        let _ = sender
            .send(Message::Item(ServerMessage::Error(
                "Sorry, there's a problem with your credentials.".to_string(),
            )))
            .await;

        return;
    } else {
        info!("welcome");

        let _ = sender.send(Message::Item(ServerMessage::Welcome {})).await;
    }

    // Consider handing off to another method here.
    let session = session.unwrap();
    let our_key = session.key.to_string();

    let (session_tx, _session_rx) = broadcast::channel::<ServerMessage>(10);

    // Forward global events to the client if they're the intended audience.
    let mut rx = state.tx.subscribe();
    let broadcasting_tx = session_tx.clone();
    let mut broadcasting_task = tokio::spawn(async move {
        while let Ok(server_message) = rx.recv().await {
            match &server_message {
                ServerMessage::Notify(key, _) => {
                    if our_key == *key {
                        if broadcasting_tx.send(server_message).is_err() {
                            warn!("broadcasting:tx:error");
                            break;
                        }
                    }
                }
                ignoring => warn!("brodcasted:ignoring {:?}", ignoring),
            };
        }
    });

    // Send all outgoing traffic to the client, either from forwarded global
    // events or replies to commands.
    let mut session_rx = session_tx.subscribe();
    let mut send_task = tokio::spawn(async move {
        while let Ok(server_message) = session_rx.recv().await {
            // In any websocket error, break loop.
            if sender.send(Message::Item(server_message)).await.is_err() {
                warn!("sending:tx:error");
                break;
            }
        }
    });

    let name = session.name.clone();
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

                    session.close(app_state).expect("Error closing session");

                    // Forward to send task.
                    let _ = session_tx.send(ServerMessage::Reply(
                        reply.to_json().expect("Errror serializing reply JSON"),
                    ));
                }
                _ => todo!(),
            }
        }
    });

    // If any one of the tasks exit, abort the others.
    tokio::select! {
        _ = (&mut broadcasting_task) => warn!("broadcasting-task:exited"),
        _ = (&mut send_task) => {
            info!("send-task:exited");
            recv_task.abort()
        },
        _ = (&mut recv_task) => {
            info!("recv-task:exited");
            send_task.abort();
        },
    };

    // TODO Send user left message, once we can determine the criteria. I'm
    // thinking we need to have a cool down period just in case they reload the
    // tab or something.

    state.remove_session(&session);
}
