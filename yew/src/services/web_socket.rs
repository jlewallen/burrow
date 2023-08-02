use futures::{
    channel::mpsc::{Sender, TrySendError},
    SinkExt, StreamExt,
};
use gloo_timers::future::TimeoutFuture;
use reqwasm::websocket::{futures::WebSocket, Message};
use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use wasm_bindgen_futures::spawn_local;
use yew::Callback;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WebSocketMessage {
    Token { token: String },
    Welcome { self_key: String },
    Evaluate(String),
    Reply(serde_json::Value),
    Notify((String, serde_json::Value)),
}

#[derive(Debug)]
pub enum ReceivedMessage {
    Connecting,
    Item(String),
}

#[derive(Clone)]
struct ActiveConnection {
    tx: Sender<Option<String>>,
    busy: Arc<AtomicBool>,
}

impl ActiveConnection {
    fn new(incoming: Callback<ReceivedMessage>) -> Self {
        let (in_tx, mut in_rx) = futures::channel::mpsc::channel::<Option<String>>(100);

        log::debug!("ws:new");

        // This needs to have a shorter timeout.
        let ws = WebSocket::open("ws://127.0.0.1:3000/ws").unwrap();
        let (mut write, mut read) = ws.split();

        log::debug!("ws:opened");

        let busy = Arc::new(AtomicBool::new(true));
        let check_busy = Arc::clone(&busy);

        spawn_local(async move {
            log::debug!("ws:tx-open");

            while let Some(s) = in_rx.next().await {
                match s {
                    Some(s) => {
                        log::debug!("ws:send {}", s);
                        write.send(Message::Text(s)).await.unwrap();
                    }
                    None => break,
                }
            }

            busy.store(false, Ordering::SeqCst);

            log::debug!("ws:tx-close");
        });

        let closer = in_tx.clone();

        spawn_local(async move {
            log::debug!("ws:rx-open");

            while let Some(msg) = read.next().await {
                match msg {
                    Ok(Message::Text(data)) => {
                        log::trace!("ws:text {}", data);
                        incoming.emit(ReceivedMessage::Item(data))
                    }
                    Ok(Message::Bytes(b)) => {
                        let decoded = std::str::from_utf8(&b);
                        if let Ok(val) = decoded {
                            log::trace!("ws:bytes {}", &val);
                            incoming.emit(ReceivedMessage::Item(val.into()))
                        }
                    }
                    Err(e) => {
                        log::error!("ws: {:?}", e)
                    }
                }
            }

            closer.clone().send(None).await.unwrap();

            log::debug!("ws:rx-close");
        });

        Self {
            tx: in_tx,
            busy: check_busy,
        }
    }

    fn try_send(&self, value: String) -> Result<(), TrySendError<Option<String>>> {
        self.tx.clone().try_send(Some(value))
    }

    fn is_busy(&self) -> bool {
        self.busy.load(Ordering::SeqCst)
    }
}

#[derive(Clone)]
pub struct WebSocketService {
    connection: Arc<RefCell<Option<ActiveConnection>>>,
}

impl WebSocketService {
    pub fn new(incoming: Callback<ReceivedMessage>) -> Self {
        let connection = Arc::new(RefCell::new(None::<ActiveConnection>));
        let sender = Arc::clone(&connection);

        spawn_local(async move {
            loop {
                let connecting = {
                    let mut c = connection.borrow_mut();
                    let reconnecting = match &*c {
                        Some(c) => !c.is_busy(),
                        _ => true,
                    };
                    if reconnecting {
                        log::debug!("connecting");

                        *c = Some(ActiveConnection::new(incoming.clone()));
                        true
                    } else {
                        false
                    }
                };

                if connecting {
                    incoming.emit(ReceivedMessage::Connecting);
                }

                TimeoutFuture::new(1_000).await;
            }
        });

        Self { connection: sender }
    }

    pub fn try_send(&self, value: String) -> Result<(), TrySendError<Option<String>>> {
        self.connection
            .as_ref()
            .borrow()
            .as_ref()
            .map(|c| c.try_send(value).unwrap())
            .unwrap();

        Ok(())
    }
}
