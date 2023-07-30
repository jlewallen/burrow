use anyhow::Result;
use axum::{
    extract::Extension,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, get_service, post},
    Json, Router,
};
use axum_typed_websockets::{Message, WebSocket, WebSocketUpgrade};
use chrono::{DateTime, Utc};
use clap::Args;
use futures::{sink::SinkExt, stream::StreamExt};
use serde::{Deserialize, Serialize};
use std::{
    borrow::Borrow, collections::HashMap, net::SocketAddr, ops::Sub, path::PathBuf, rc::Rc,
    sync::Arc,
};
use tokio::{signal, sync::Mutex, task::JoinHandle};
use tokio::{sync::broadcast, time::sleep};
use tower_http::{
    services::ServeDir,
    trace::{DefaultMakeSpan, TraceLayer},
};
use tracing::{debug, info, warn};

use engine::{AfterTick, DevNullNotifier, Domain, HasUsernames, Notifier, Session, SessionOpener};
use kernel::{DomainEvent, Effect, EntityKey, EntryResolver, SimpleReply};
use replies::ToJson;

use crate::{make_domain, PluginConfiguration};

#[derive(Debug, Args)]
pub struct Command {}

impl Command {
    fn plugin_configuration(&self) -> PluginConfiguration {
        PluginConfiguration::default()
    }
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
enum ServerMessage {
    Error(String),
    Welcome { self_key: String },
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
    tick_deadline: Mutex<Option<DateTime<Utc>>>,
    tx: broadcast::Sender<ServerMessage>,
}

impl AppState {
    pub fn try_start_session(&self, name: &str, key: &EntityKey) -> Result<ClientSession> {
        Ok(ClientSession {
            name: name.to_string(),
            key: key.clone(),
        })
    }

    pub async fn tick(&self, now: DateTime<Utc>) -> Result<AfterTick> {
        let can_tick = {
            let tick_deadline = self.tick_deadline.lock().await;

            tick_deadline.filter(|deadline| *deadline > now)
        };

        match can_tick {
            Some(deadline) => Ok(AfterTick::Deadline(deadline)),
            None => {
                Ok(self.domain.tick(now, &self.notifier())?)

                /*
                match maybe_deadline {
                    Some(deadline) => {
                        let mut tick_deadline = self.tick_deadline.lock().await;
                        *tick_deadline = Some(deadline.clone());

                        Ok(AfterTick::Deadline(deadline.clone()))
                    }
                    None => Ok(AfterTick::Flushed),
                }*/
            }
        }
    }

    fn notifier(&self) -> SenderNotifier {
        SenderNotifier {
            tx: self.tx.clone(),
        }
    }

    fn find_user_key(&self, name: &str) -> Result<Option<EntityKey>> {
        let session = self.domain.open_session().expect("Error opening session");

        let world = session.world()?.expect("No world");
        let maybe_key = world.find_name_key(name)?;

        session.close(&DevNullNotifier::default())?;

        Ok(maybe_key)
    }

    fn remove_session(&self, _session: &ClientSession) {}
}

struct SenderNotifier {
    tx: broadcast::Sender<ServerMessage>,
}

impl Notifier for SenderNotifier {
    fn notify(&self, audience: &EntityKey, observed: &Rc<dyn DomainEvent>) -> Result<()> {
        debug!("notify {:?} -> {:?}", audience, observed);

        let serialized = observed.to_json()?;
        let outgoing = ServerMessage::Notify(audience.to_string(), serialized);
        self.tx.send(outgoing)?;

        Ok(())
    }
}

#[tokio::main]
pub async fn execute_command(cmd: &Command) -> Result<()> {
    info!("serving");

    let domain = make_domain(cmd.plugin_configuration()).await?;

    let assets_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");

    let (tx, _rx) = broadcast::channel(100);

    let app_state = Arc::new(AppState {
        domain: domain.clone(),
        tick_deadline: Default::default(),
        tx,
    });

    let notifier = app_state.notifier();

    let app = Router::new()
        .fallback(get_service(
            ServeDir::new(assets_dir).append_index_html_on_directories(true),
        ))
        .route("/tick", post(tick_handler))
        .route("/ws", get(ws_handler))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::default().include_headers(false)),
        )
        .layer(Extension(app_state));

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    info!("listening on {}", addr);

    tokio::task::spawn({
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

fn empty_map() -> HashMap<String, String> {
    Default::default()
}

fn empty_headers() -> HeaderMap {
    Default::default()
}

fn deadline_headers(now: DateTime<Utc>, deadline: Option<DateTime<Utc>>) -> HeaderMap {
    match deadline {
        Some(deadline) => {
            let mut headers = HeaderMap::new();
            let remaining = deadline.sub(now);
            let remaining = format!("{:?}", remaining.num_milliseconds());
            headers.insert("retry-after", format!("{:?}", deadline).parse().unwrap());
            headers.insert("retry-delay-ms", remaining.parse().unwrap());
            headers
        }
        None => {
            let mut headers = HeaderMap::new();
            let remaining = format!("{}", 1000);
            headers.insert("retry-delay-ms", remaining.parse().unwrap());
            headers
        }
    }
}

async fn tick_handler(Extension(state): Extension<Arc<AppState>>) -> impl IntoResponse {
    let now = Utc::now();
    match state.tick(Utc::now()).await {
        Ok(AfterTick::Processed(_)) => {
            info!("tick:processed");

            (StatusCode::OK, empty_headers(), Json(empty_map()))
        }
        Ok(AfterTick::Deadline(deadline)) => {
            info!(%deadline, "tick:too-many");

            (
                StatusCode::TOO_MANY_REQUESTS,
                deadline_headers(now, Some(deadline)),
                Json(empty_map()),
            )
        }
        Ok(AfterTick::Empty) => {
            info!("tick:empty");

            (
                StatusCode::TOO_MANY_REQUESTS,
                deadline_headers(now, None),
                Json(empty_map()),
            )
        }
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            empty_headers(),
            Json(empty_map()),
        ),
    }
}

async fn ws_handler(
    ws: WebSocketUpgrade<ServerMessage, ClientMessage>,
    Extension(state): Extension<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

fn evaluate_commands<T>(
    session: Rc<Session>,
    notifier: &T,
    name: &str,
    text: &str,
) -> Result<Effect>
where
    T: Notifier,
{
    let effect: Effect = if let Some(effect) = session
        .evaluate_and_perform(name, text)
        .expect("Evaluation error")
    {
        effect
    } else {
        SimpleReply::What.into()
    };

    session.close(notifier).expect("Error closing session");

    Ok(effect)
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

    if let Some(session) = &session {
        info!("welcome");

        let _ = sender
            .send(Message::Item(ServerMessage::Welcome {
                self_key: session.key.to_string(),
            }))
            .await;
    } else {
        info!("bad credentials");

        let _ = sender
            .send(Message::Item(ServerMessage::Error(
                "Sorry, there's a problem with your credentials.".to_string(),
            )))
            .await;

        return;
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
                    if our_key == *key && broadcasting_tx.send(server_message).is_err() {
                        warn!("broadcasting:tx:error");
                        break;
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

                    let handle: JoinHandle<Result<serde_json::Value>> =
                        tokio::task::spawn_blocking({
                            let domain = app_state.domain.clone();
                            let notifier = app_state.notifier();
                            let name = name.clone();
                            let text = text.clone();

                            move || {
                                let session = domain.open_session().expect("Error opening session");
                                let effect = evaluate_commands(session, &notifier, &name, &text)?;
                                Ok(effect.to_json()?)
                            }
                        });

                    match handle.await {
                        Ok(Ok(reply)) => session_tx
                            .send(ServerMessage::Reply(reply))
                            .expect("Error sending reply"),
                        Ok(Err(_)) => todo!(),
                        Err(_) => todo!(),
                    };
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
