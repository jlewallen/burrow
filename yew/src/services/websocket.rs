use std::cell::RefCell;

use crate::services::event_bus::{EventBus, Request};
use futures::{
    channel::mpsc::{Sender, TrySendError},
    SinkExt, StreamExt,
};
use reqwasm::websocket::{futures::WebSocket, Message};
use serde::{Deserialize, Serialize};
use wasm_bindgen_futures::spawn_local;
use yew_agent::Dispatched;

#[derive(Debug, Serialize, Deserialize)]
pub struct Login {
    username: String,
    password: String,
}

struct ActiveConnection {
    tx: Sender<String>,
}

impl ActiveConnection {
    fn new() -> Self {
        let (in_tx, mut in_rx) = futures::channel::mpsc::channel::<String>(1000);
        let mut event_bus = EventBus::dispatcher();

        let ws = WebSocket::open("ws://127.0.0.1:3000/ws").unwrap();
        let (mut write, mut read) = ws.split();

        spawn_local(async move {
            // log::debug!("ws:tx-begin");

            while let Some(s) = in_rx.next().await {
                log::debug!("ws:send {}", s);
                write.send(Message::Text(s)).await.unwrap();
            }

            log::debug!("ws:tx-close");
        });

        spawn_local(async move {
            // log::debug!("ws:rx-begin");

            while let Some(msg) = read.next().await {
                match msg {
                    Ok(Message::Text(data)) => {
                        log::debug!("ws:text {}", data);
                        event_bus.send(Request::EventBusMsg(data));
                    }
                    Ok(Message::Bytes(b)) => {
                        let decoded = std::str::from_utf8(&b);
                        if let Ok(val) = decoded {
                            log::debug!("ws:bytes {}", val);
                            event_bus.send(Request::EventBusMsg(val.into()));
                        }
                    }
                    Err(e) => {
                        log::error!("ws: {:?}", e)
                    }
                }
            }

            log::debug!("ws:rx-closed");
        });

        Self { tx: in_tx }
    }

    fn try_send(&self, value: String) -> Result<(), TrySendError<String>> {
        self.tx.clone().try_send(value)
    }
}

pub struct WebsocketService {
    connection: RefCell<ActiveConnection>,
}

impl WebsocketService {
    pub fn new() -> Self {
        Self {
            connection: RefCell::new(ActiveConnection::new()),
        }
    }

    pub fn try_send(&self, value: String) -> Result<(), TrySendError<String>> {
        self.connection.borrow().try_send(value)
    }
}
