use std::rc::Rc;
use yew::{prelude::*, Children};
use yewdux::prelude::*;
// use gloo_console as console;

use crate::history::SessionHistory;
use crate::services::{ReceivedMessage, WebSocketMessage, WebSocketService};

#[derive(Debug, Clone, PartialEq)]
pub struct Evaluator {
    pub callback: Callback<String>,
}

#[derive(Properties, Clone, PartialEq)]
pub struct Props {
    pub children: Children,
}

pub enum Msg {
    Received(ReceivedMessage),
    Evaluate(String),
}

pub struct AlwaysOpenWebSocket {
    self_key: Option<String>,
    wss: WebSocketService,
    evaluator: Evaluator,
}

impl Component for AlwaysOpenWebSocket {
    type Message = Msg;
    type Properties = Props;

    fn create(ctx: &Context<Self>) -> Self {
        let receive_callback = ctx
            .link()
            .callback(|m: ReceivedMessage| Self::Message::Received(m));
        let evaluate_callback = ctx.link().callback(|m: String| Self::Message::Evaluate(m));

        let wss = WebSocketService::new(receive_callback);

        Self {
            wss,
            self_key: None,
            evaluator: Evaluator {
                callback: evaluate_callback,
            },
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Self::Message::Received(ReceivedMessage::Connecting) => {
                self.wss
                    .try_send(
                        serde_json::to_string(&WebSocketMessage::Login {
                            username: "jlewallen".into(),
                            password: "jlewallen".into(),
                        })
                        .unwrap(),
                    )
                    .unwrap();

                true
            }
            Self::Message::Received(ReceivedMessage::Item(value)) => {
                match serde_json::from_str::<WebSocketMessage>(&value).unwrap() {
                    WebSocketMessage::Welcome { self_key } => {
                        self.self_key = Some(self_key);
                        self.wss
                            .try_send(
                                serde_json::to_string(&WebSocketMessage::Evaluate("look".into()))
                                    .unwrap(),
                            )
                            .unwrap();

                        false
                    }
                    WebSocketMessage::Reply(value) => {
                        let dispatch = Dispatch::<SessionHistory>::new();

                        dispatch.reduce(move |history| Rc::new(history.append(value)));

                        true
                    }
                    WebSocketMessage::Notify((key, value)) => {
                        let dispatch = Dispatch::<SessionHistory>::new();

                        log::debug!("notify: key={:?}", key);

                        dispatch.reduce(move |history| Rc::new(history.append(value)));

                        true
                    }
                    _ => false,
                }
            }
            Self::Message::Evaluate(value) => {
                self.wss
                    .try_send(serde_json::to_string(&WebSocketMessage::Evaluate(value)).unwrap())
                    .unwrap();

                false
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        html! {
            <ContextProvider<Evaluator> context={self.evaluator.clone()}>
                { for ctx.props().children.iter() }
            </ContextProvider<Evaluator>>
        }
    }
}
