use gloo_console as console;
use serde::{Deserialize, Serialize};
use yew_agent::utils::store::{Store, StoreWrapper};
use yew_agent::AgentLink;

use crate::services::websocket::WebsocketService;

pub type EntryId = u32;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct HistoryEntry {
    pub id: EntryId,
    pub text: String,
}

#[derive(Debug)]
pub enum PostRequest {
    Send(String),
}

#[derive(Debug)]
pub enum Action {
    Send(String),
}

pub struct HistoryStore {
    pub entries: Vec<HistoryEntry>,
    pub wss: WebsocketService,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WebSocketMessage {
    Login { username: String, password: String },
    Evaluate(String),
}

impl Store for HistoryStore {
    type Action = Action;
    type Input = PostRequest;

    fn new() -> Self {
        let mut entries: Vec<HistoryEntry> = Vec::new();

        entries.push(HistoryEntry {
            id: 0,
            text: "Hello, world!".into(),
        });

        entries.push(HistoryEntry {
            id: 1,
            text: "How are you?!".into(),
        });

        let wss = WebsocketService::new();

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
        match msg {
            PostRequest::Send(text) => link.send_message(Action::Send(text)),
        }
    }

    fn reduce(&mut self, msg: Self::Action) {
        match msg {
            Action::Send(text) => {
                console::log!("Sending", &text);

                let message = WebSocketMessage::Evaluate(text);

                if let Ok(_) = self.wss.try_send(serde_json::to_string(&message).unwrap()) {
                    log::debug!("message sent successfully");
                }
            }
        }
    }
}

impl HistoryStore {}
