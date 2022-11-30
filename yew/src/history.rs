use std::cell::RefCell;
use std::sync::Arc;

// use crate::services::event_bus::{EventBus, Request};
use crate::services::web_socket::{ReceivedMessage, WebSocketService};
use futures::StreamExt;
use gloo_console as console;
use serde::{Deserialize, Serialize};
use wasm_bindgen_futures::spawn_local;
use yew::Callback;
use yew_agent::utils::store::{Bridgeable, Store, StoreWrapper};
use yew_agent::{AgentLink, Bridged, Dispatched};

pub type EntryId = u32;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct HistoryEntry {
    pub id: EntryId,
    pub text: String,
}

impl HistoryEntry {
    pub fn new(_reply: serde_json::Value) -> Self {
        Self {
            id: 0,
            text: "Hello".into(),
        }
    }
}

#[derive(Debug)]
pub enum PostRequest {
    Send(String),
    Reply(serde_json::Value),
}

#[derive(Debug)]
pub enum Action {
    Send(String),
    Append(String),
}

pub struct HistoryStore {
    pub entries: Arc<RefCell<Vec<HistoryEntry>>>,
    pub wss: WebSocketService,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WebSocketMessage {
    Login { username: String, password: String },
    Evaluate(String),
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
enum ServerMessage {
    Error(String),
    Welcome {},
    Reply(serde_json::Value),
}

impl Store for HistoryStore {
    type Action = Action;
    type Input = PostRequest;

    fn new() -> Self {
        let entries: Arc<RefCell<Vec<HistoryEntry>>> = Arc::new(RefCell::new(Vec::new()));
        let appending = Arc::clone(&entries);

        let (incoming_tx, mut incoming_rx) =
            futures::channel::mpsc::channel::<Option<ReceivedMessage>>(100);

        let wss = WebSocketService::new(incoming_tx);
        let first_look = wss.clone();

        /*
        let mut event_bus = EventBus::bridge(Callback::from(|m: String| {
            let mut d = HistoryStore::bridge(Callback::from(|_| {
                log::debug!("message:ignored");
            }));

            log::debug!("ok {}", &m);

            d.send(PostRequest::Reply(serde_json::to_value(&m).unwrap()))
        }));
        */

        spawn_local(async move {
            log::debug!("history:open");

            while let Some(s) = incoming_rx.next().await {
                match s {
                    Some(s) => match s {
                        ReceivedMessage::Text(value) => match value {
                            serde_json::Value::String(text) => {
                                let message: ServerMessage = serde_json::from_str(&text).unwrap();
                                match message {
                                    ServerMessage::Error(_) => todo!(),
                                    ServerMessage::Welcome {} => {
                                        log::debug!("connected");

                                        let message = WebSocketMessage::Evaluate("look".into());
                                        if let Ok(_) = first_look
                                            .try_send(serde_json::to_string(&message).unwrap())
                                        {
                                            log::debug!("message sent successfully");
                                        }
                                    }
                                    ServerMessage::Reply(reply) => {
                                        log::debug!("reply: {:?}", reply);

                                        let mut appending = appending.borrow_mut();
                                        appending.push(HistoryEntry::new(reply.clone()));
                                    }
                                }
                            }
                            _ => {
                                log::debug!("received");
                            }
                        },
                    },
                    None => break,
                }
            }

            log::debug!("history:close");
        });

        let message = WebSocketMessage::Login {
            username: "jlewallen".into(),
            password: "jlewallen".into(),
        };

        if let Ok(_) = wss.try_send(serde_json::to_string(&message).unwrap()) {
            log::debug!("message sent successfully");
        }

        Self { entries, wss }
    }

    fn handle_input(&self, link: AgentLink<StoreWrapper<Self>>, msg: Self::Input) {
        log::debug!("history:message");
        match msg {
            PostRequest::Send(text) => link.send_message(Action::Send(text)),
            PostRequest::Reply(_) => {
                log::debug!("REPLY");
            }
        }
    }

    fn reduce(&mut self, msg: Self::Action) {
        match msg {
            Action::Send(text) => {
                console::log!("sending", &text);

                let message = WebSocketMessage::Evaluate(text);

                if let Ok(_) = self.wss.try_send(serde_json::to_string(&message).unwrap()) {
                    log::debug!("message sent successfully");
                }
            }
            Action::Append(text) => {
                console::log!("appending", &text);
            }
        }
    }
}

impl HistoryStore {}
