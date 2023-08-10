use anyhow::Result;
use axum::{extract::Extension, response::IntoResponse};
use axum_typed_websockets::{Message, WebSocket, WebSocketUpgrade};
use futures::{
    sink::SinkExt,
    stream::{SplitStream, StreamExt},
};
use serde::{Deserialize, Serialize};
use std::{borrow::Borrow, rc::Rc, sync::Arc};
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tracing::{info, trace, warn};

use engine::{EvaluateAs, Notifier, Session, SessionOpener};
use kernel::common::SimpleReply;
use kernel::prelude::{
    Effect, EntityKey, EntryResolver, JsonValue, LookupBy, Perform, PerformAction, Performer,
};

use crate::{
    rpc::try_parse_action,
    serve::{handlers::TokenClaims, ClientSession},
};

use super::AppState;

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum ClientMessage {
    Token { token: String },
    Evaluate(String),
    Perform(JsonValue),
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub enum ServerMessage {
    Error(String),
    Welcome { self_key: String },
    Reply(JsonValue),
    Notify(String, JsonValue),
}

pub async fn ws_handler(
    ws: WebSocketUpgrade<ServerMessage, ClientMessage>,
    Extension(state): Extension<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn wait_for_token(
    receiver: &mut SplitStream<WebSocket<ServerMessage, ClientMessage>>,
    state: &Arc<AppState>,
) -> Result<ClientSession> {
    while let Some(Ok(m)) = receiver.next().await {
        match m {
            Message::Item(ClientMessage::Token { token }) => {
                let claims = jsonwebtoken::decode::<TokenClaims>(
                    &token,
                    &jsonwebtoken::DecodingKey::from_secret(state.env.jwt_secret.as_ref()),
                    &jsonwebtoken::Validation::default(),
                )?;

                let key = claims.claims.sub;

                return Ok(state
                    .try_start_session(&EntityKey::new(&key))
                    .expect("Error starting session"));
            }
            m => {
                info!("unexpected: {:?}", m);
            }
        }
    }

    Err(anyhow::anyhow!("No token"))
}

async fn handle_socket(stream: WebSocket<ServerMessage, ClientMessage>, state: Arc<AppState>) {
    let (mut sender, mut receiver) = stream.split();

    info!("authenticating");
    let Ok(session) = wait_for_token(&mut receiver, &state).await else {
            return;
    };

    info!("welcome");
    let _ = sender
        .send(Message::Item(ServerMessage::Welcome {
            self_key: session.key.to_string(),
        }))
        .await;

    let our_key = session.key.to_string();
    let (session_tx, _session_rx) = broadcast::channel::<ServerMessage>(10);

    // Forward global events to the client if they're the intended audience.
    let mut rx = state.tx.subscribe();
    let broadcasting_tx = session_tx.clone();
    let mut broadcasting_task = tokio::spawn({
        let our_key = our_key.clone();
        async move {
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

    let mut recv_task = tokio::spawn({
        let state = state.clone();
        async move {
            while let Some(Ok(Message::Item(message))) = receiver.next().await {
                trace!("item {:?}", &message);

                match message {
                    ClientMessage::Perform(value) => {
                        let app_state: &AppState = state.borrow();

                        let handle: JoinHandle<Result<JsonValue>> = tokio::task::spawn_blocking({
                            let domain = app_state.domain.clone();
                            let notifier = app_state.notifier();
                            let our_key = our_key.clone();

                            move || {
                                let session = domain.open_session().expect("Error opening session");
                                // TODO This could be a PerformAction
                                let action: Rc<_> = try_parse_action(value)
                                    .expect("try parse action failed")
                                    .into();
                                let living = session
                                    .entry(&LookupBy::Key(&EntityKey::new(&our_key)))
                                    .expect("Living lookup failed")
                                    .expect("Living not found");
                                let perform = Perform::Living {
                                    living,
                                    action: PerformAction::Instance(action),
                                };
                                trace!("perform {:?}", &perform.enum_name());
                                let session = session.set_session()?;
                                let effect = session.perform(perform).expect("Perform failed");
                                session.close(&notifier).expect("Error closing session");
                                Ok(serde_json::to_value(effect)?)
                            }
                        });

                        match handle.await {
                            Ok(Ok(reply)) => session_tx
                                .send(ServerMessage::Reply(reply))
                                .expect("Error sending reply"),
                            Ok(Err(e)) => todo!("{:?}", e),
                            Err(e) => todo!("{:?}", e),
                        };
                    }
                    ClientMessage::Evaluate(text) => {
                        let app_state: &AppState = state.borrow();

                        let handle: JoinHandle<Result<JsonValue>> = tokio::task::spawn_blocking({
                            let domain = app_state.domain.clone();
                            let notifier = app_state.notifier();
                            let text = text.clone();
                            let our_key = our_key.clone();

                            move || {
                                let session = domain.open_session().expect("Error opening session");
                                let effect = evaluate_commands(
                                    session,
                                    &notifier,
                                    EvaluateAs::Key(&EntityKey::new(&our_key)),
                                    &text,
                                )?;
                                Ok(serde_json::to_value(effect)?)
                            }
                        });

                        match handle.await {
                            Ok(Ok(reply)) => {
                                session_tx
                                    .send(ServerMessage::Reply(reply))
                                    .expect("Error sending reply");
                            }
                            Ok(Err(e)) => warn!("{:?}", e),
                            Err(e) => warn!("{:?}", e),
                        };
                    }
                    _ => todo!(),
                }
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

fn evaluate_commands<T>(
    session: Rc<Session>,
    notifier: &T,
    eval_as: EvaluateAs,
    text: &str,
) -> Result<Effect>
where
    T: Notifier,
{
    let effect: Effect = if let Some(effect) = session
        .evaluate_and_perform_as(eval_as, text)
        .expect("Evaluation error")
    {
        effect
    } else {
        SimpleReply::What.try_into()?
    };

    session.close(notifier).expect("Error closing session");

    Ok(effect)
}
