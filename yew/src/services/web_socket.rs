use dotenv_codegen::dotenv;
use futures::{
    channel::mpsc::{Sender, TrySendError},
    SinkExt, StreamExt,
};
use futures_util::future::{select, Either};
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
    Perform(serde_json::Value),
    Reply(serde_json::Value),
    Notify((String, serde_json::Value)),
    Error(String),
}

#[derive(Debug)]
pub enum ReceivedMessage {
    Item(String),
}

#[derive(Clone)]
struct ActiveConnection {
    tx: Sender<Option<String>>,
    busy: Arc<AtomicBool>,
}

const WS_URL: &str = dotenv!("WS_URL");

impl ActiveConnection {
    fn new(incoming: Callback<(Sender<Option<String>>, ReceivedMessage)>) -> Self {
        let (in_tx, mut in_rx) = futures::channel::mpsc::channel::<Option<String>>(100);

        log::trace!("ws:new");

        // This needs to have a shorter timeout.
        let ws = WebSocket::open(WS_URL).unwrap();
        let (mut write, mut read) = ws.split();

        log::trace!("ws:opened");

        let busy = Arc::new(AtomicBool::new(true));
        let check_busy = Arc::clone(&busy);

        spawn_local(async move {
            log::trace!("ws:tx-open");

            while let Some(s) = in_rx.next().await {
                let ok = match s {
                    Some(s) => {
                        let to = TimeoutFuture::new(1_000);
                        match select(to, write.send(Message::Text(s))).await {
                            Either::Right((_, _)) => Some(true),
                            Either::Left((_, b)) => {
                                drop(b);
                                None
                            }
                        }
                    }
                    None => None,
                };

                if ok.is_none() {
                    break;
                }
            }

            busy.store(false, Ordering::SeqCst);

            log::trace!("ws:tx-close");
        });

        let closer = in_tx.clone();

        spawn_local({
            let in_tx = in_tx.clone();
            async move {
                log::trace!("ws:rx-open");

                while let Some(msg) = read.next().await {
                    match msg {
                        Ok(Message::Text(data)) => {
                            log::trace!("ws:text {}", data);
                            incoming.emit((in_tx.clone(), ReceivedMessage::Item(data)))
                        }
                        Ok(Message::Bytes(b)) => {
                            let decoded = std::str::from_utf8(&b);
                            if let Ok(val) = decoded {
                                log::trace!("ws:bytes {}", &val);
                                incoming.emit((in_tx.clone(), ReceivedMessage::Item(val.into())))
                            }
                        }
                        Err(e) => {
                            log::error!("ws: {:?}", e)
                        }
                    }
                }

                log::trace!("ws:rx-closing");

                match closer.clone().send(None).await {
                    Err(e) => log::warn!("ws:close-error: {:?}", e),
                    Ok(_) => {}
                };

                log::trace!("ws:rx-close");
            }
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
#[allow(dead_code)]
pub struct WebSocketService {
    connection: Arc<RefCell<Option<ActiveConnection>>>,
}

impl WebSocketService {
    pub fn new(
        first: Option<String>,
        incoming: Callback<(Sender<Option<String>>, ReceivedMessage)>,
    ) -> Self {
        let connection = Arc::new(RefCell::new(None::<ActiveConnection>));
        let sender = Arc::clone(&connection);

        spawn_local(async move {
            loop {
                let _connecting = {
                    let mut c = connection.borrow_mut();
                    let reconnecting = match &*c {
                        Some(c) => !c.is_busy(),
                        _ => true,
                    };
                    if reconnecting {
                        log::debug!("connecting");

                        let starting = ActiveConnection::new(incoming.clone());

                        if let Some(first) = &first {
                            starting
                                .try_send(first.clone())
                                .expect("ws send first failed");
                        }

                        *c = Some(starting);
                        true
                    } else {
                        false
                    }
                };

                TimeoutFuture::new(1_000).await;
            }
        });

        Self { connection: sender }
    }

    #[allow(dead_code)]
    pub fn try_send(&self, value: String) -> Result<(), TrySendError<Option<String>>> {
        log::trace!("sending {:?}", value);
        self.connection
            .as_ref()
            .borrow()
            .as_ref()
            .map(|c| c.try_send(value).unwrap())
            .unwrap();

        Ok(())
    }
}
