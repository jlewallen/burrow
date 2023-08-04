use std::{cell::RefCell, rc::Rc};
use yew::prelude::*;

mod manage_connection {
    use futures::channel::mpsc::Sender;
    use yew::functional::use_reducer;
    use yew::{prelude::*, Children};

    use crate::{
        hooks::use_user_context,
        services::{ReceivedMessage, WebSocketMessage, WebSocketService},
        shared::Evaluator,
        types::SessionHistory,
    };

    #[derive(Properties, Clone, PartialEq)]
    pub struct Props {
        pub children: Children,
    }

    /// User context provider.
    #[function_component(ManageConnection)]
    pub fn manage_connection(props: &Props) -> Html {
        let history = use_reducer(SessionHistory::default);
        let evaluator = use_state(Evaluator::default);
        let wss = use_state(|| None::<WebSocketService>);
        let user_ctx = use_user_context();

        let append = history.clone();
        let set_evaluator = evaluator.clone();
        use_effect_with_deps(
            move |(user,)| {
                if let Some(token) = user.token() {
                    log::info!("conn:authenticated");

                    let token = token.clone();
                    let first = serde_json::to_string(&WebSocketMessage::Token {
                        token: token.clone(),
                    })
                    .expect("web socket message token error");

                    let service = WebSocketService::new(Some(first), {
                        Callback::from(
                            move |(mut c, r): (Sender<Option<String>>, ReceivedMessage)| {
                                match r {
                                    ReceivedMessage::Item(item) => {
                                        // log::info!("{:?}", item);
                                        match serde_json::from_str::<WebSocketMessage>(&item)
                                            .unwrap()
                                        {
                                            WebSocketMessage::Welcome { self_key: _ } => {
                                                let reply = serde_json::to_string(
                                                    &WebSocketMessage::Evaluate("look".into()),
                                                )
                                                .unwrap();
                                                c.try_send(Some(reply))
                                                    .expect("welcome: try send failed");
                                            }
                                            WebSocketMessage::Reply(value) => {
                                                log::debug!("notify:");

                                                append.dispatch(value);
                                            }
                                            WebSocketMessage::Notify((key, value)) => {
                                                log::debug!("notify: key={:?}", key);

                                                append.dispatch(value);
                                            }
                                            _ => todo!(),
                                        };
                                    }
                                };
                            },
                        )
                    });

                    set_evaluator.set(Evaluator::new(Callback::from({
                        let service = service.clone();
                        move |value| {
                            let value =
                                serde_json::to_string(&WebSocketMessage::Evaluate(value)).unwrap();

                            service.try_send(value).expect("try send failed");
                        }
                    })));

                    wss.set(Some(service));
                } else {
                    wss.set(None);
                }
            },
            (user_ctx,),
        );

        html! {
            <ContextProvider<SessionHistory> context={(*history).clone()}>
                <ContextProvider<Evaluator> context={(*evaluator).clone()}>
                { for props.children.iter() }
                </ContextProvider<Evaluator>>
            </ContextProvider<SessionHistory>>
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Evaluator {
    callback: Rc<RefCell<Option<Callback<String>>>>,
}

impl Evaluator {
    pub fn new(callback: Callback<String>) -> Self {
        Self {
            callback: Rc::new(RefCell::new(Some(callback))),
        }
    }

    pub fn evaluate(&self, line: String) -> () {
        let callback = self.callback.borrow();
        match callback.as_ref() {
            Some(callback) => callback.emit(line),
            None => todo!(),
        };
    }
}

pub type AlwaysOpenWebSocket = manage_connection::ManageConnection;
