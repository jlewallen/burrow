use crate::services::{ReceivedMessage, WebSocketMessage, WebSocketService};
// use gloo_console as console;
use internal::*;
use std::rc::Rc;
use yew::{prelude::*, Children};
use yewdux::prelude::*;

pub enum Msg {
    Send(String),
}

#[derive(Properties, Clone, PartialEq)]
pub struct WebSocketProps {
    pub children: Children,
}

pub struct AlwaysOpenWebSocket {
    wss: WebSocketService,
}

impl Component for AlwaysOpenWebSocket {
    type Message = ReceivedMessage;

    type Properties = WebSocketProps;

    fn create(ctx: &Context<Self>) -> Self {
        let callback = ctx.link().callback(|m: ReceivedMessage| m);

        let wss = WebSocketService::new(callback);

        Self { wss }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            ReceivedMessage::Connecting => {
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
            ReceivedMessage::Item(value) => {
                match serde_json::from_str::<WebSocketMessage>(&value).unwrap() {
                    WebSocketMessage::Welcome {} => {
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
                    _ => false,
                }
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        html! {
            { for ctx.props().children.iter() }
        }
    }
}

pub struct Home {}

impl Component for Home {
    type Message = Msg;
    type Properties = ();

    fn create(_ctx: &Context<Self>) -> Self {
        Self {}
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            _ => false,
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let link = ctx.link();
        html! {
            <div id="hack">
                <div id="upper">

                    <div id="main"><History /></div>
                </div>
                <div id="lower">
                    <div class="interactables">
                        <div class="editor" style="display: none;">
                            <div class="">
                                { "Tabs" }
                            </div>
                            <div class="buttons">
                                { "Buttons" }
                            </div>
                        </div>
                        <LineEditor onsubmit={link.callback(|text| Msg::Send(text))} />
                    </div>
                </div>
            </div>
        }
    }
}

mod internal {
    use crate::text_input::TextInput;
    use std::rc::Rc;
    // use gloo_console as console;
    use yew::prelude::*;
    use yewdux::prelude::*;

    pub type EntryId = u32;

    #[derive(Debug, Clone, Eq, PartialEq)]
    pub struct HistoryEntry {
        pub id: EntryId,
        pub text: String,
    }

    impl HistoryEntry {
        pub fn new(reply: serde_json::Value) -> Self {
            Self {
                id: 0,
                text: reply.to_string(),
            }
        }
    }

    #[derive(Default, Store, PartialEq)]
    pub struct SessionHistory {
        entries: Vec<HistoryEntry>,
    }

    impl SessionHistory {
        pub fn append(&self, value: serde_json::Value) -> Self {
            let mut ugly_clone = self.entries.clone();
            ugly_clone.push(HistoryEntry::new(value));
            Self {
                entries: ugly_clone,
            }
        }
    }

    #[derive(Properties, Clone, PartialEq)]
    pub struct Props {
        pub entry: HistoryEntry,
    }

    pub struct HistoryEntryItem {}

    impl Component for HistoryEntryItem {
        type Message = Msg;
        type Properties = Props;

        fn create(_ctx: &Context<Self>) -> Self {
            Self {}
        }

        fn update(&mut self, _ctx: &Context<Self>, _msg: Self::Message) -> bool {
            false
        }

        fn view(&self, ctx: &Context<Self>) -> Html {
            html! {
                <div class="entry">
                    { ctx.props().entry.text.as_str() }
                </div>
            }
        }
    }

    pub enum Msg {
        UpdateHistory(std::rc::Rc<SessionHistory>),
        Send(String),
    }

    #[derive(Properties, Clone, PartialEq)]
    pub struct HistoryProps {}

    pub struct History {
        history: Rc<SessionHistory>,
        #[allow(dead_code)]
        dispatch: Dispatch<SessionHistory>,
    }

    impl Component for History {
        type Message = Msg;
        type Properties = HistoryProps;

        fn create(ctx: &Context<Self>) -> Self {
            let callback = ctx.link().callback(Msg::UpdateHistory);
            let dispatch = Dispatch::<SessionHistory>::subscribe(move |h| callback.emit(h));

            Self {
                history: dispatch.get(),
                dispatch,
            }
        }

        fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
            match msg {
                Msg::UpdateHistory(history) => {
                    self.history = history;

                    true
                }
                _ => false,
            }
        }

        fn view(&self, _ctx: &Context<Self>) -> Html {
            html! {
                <div class="history">
                    <div class="entries">
                        { for self.history.entries.iter().map(|entry| html!{ <HistoryEntryItem entry={entry.clone()} /> }) }
                    </div>
                </div>
            }
        }
    }

    #[derive(Properties, Clone, PartialEq)]
    pub struct LineEditorProps {
        pub onsubmit: Callback<String>,
    }

    pub struct LineEditor {}

    impl Component for LineEditor {
        type Message = Msg;
        type Properties = LineEditorProps;

        fn create(_ctx: &Context<Self>) -> Self {
            Self {}
        }

        fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
            match msg {
                Msg::Send(text) => {
                    ctx.props().onsubmit.emit(text);
                    false
                }
                _ => todo!(),
            }
        }

        fn view(&self, ctx: &Context<Self>) -> Html {
            let link = ctx.link();
            html! {
                <div class="line-editor">
                    <TextInput value="" onsubmit={link.callback(|text| Msg::Send(text))} />
                </div>
            }
        }
    }
}
