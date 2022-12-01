use crate::services::{ReceivedMessage, WebSocketMessage, WebSocketService};
use gloo_console as console;
use gloo_timers::callback::Timeout;
use internal::*;
use std::rc::Rc;
use web_sys::HtmlElement;
use yew::{prelude::*, Children};
use yewdux::prelude::*;

pub enum Msg {
    UpdateHistory(std::rc::Rc<SessionHistory>),
    Refresh,
    Ignored,
}

#[derive(Properties, Clone, PartialEq)]
pub struct WebSocketProps {
    pub children: Children,
}

#[derive(Debug, Clone, PartialEq)]
struct Evaluator {
    pub callback: Callback<String>,
}

pub struct AlwaysOpenWebSocket {
    wss: WebSocketService,
    evaluator: Evaluator,
}

pub enum WebSocketComponentMsg {
    Received(ReceivedMessage),
    Evaluate(String),
}

impl Component for AlwaysOpenWebSocket {
    type Message = WebSocketComponentMsg;
    type Properties = WebSocketProps;

    fn create(ctx: &Context<Self>) -> Self {
        let receive_callback = ctx
            .link()
            .callback(|m: ReceivedMessage| Self::Message::Received(m));
        let evaluate_callback = ctx.link().callback(|m: String| Self::Message::Evaluate(m));

        let wss = WebSocketService::new(receive_callback);

        Self {
            wss,
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

pub struct Home {
    refs: Vec<NodeRef>,
    evaluate_callback: Callback<String>,
    #[allow(dead_code)]
    dispatch: Dispatch<SessionHistory>,
}

impl Component for Home {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let (evaluator, _) = ctx
            .link()
            .context::<Evaluator>(ctx.link().callback(|_| Msg::Ignored))
            .expect("No evalutor context");

        let callback = ctx.link().callback(Msg::UpdateHistory);
        let dispatch = Dispatch::<SessionHistory>::subscribe(move |h| callback.emit(h));

        Self {
            refs: vec![NodeRef::default()],
            evaluate_callback: evaluator.callback.clone(),
            dispatch,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Self::Message::UpdateHistory(_) => {
                let refresher = ctx.link().callback(|_| Self::Message::Refresh);

                let timeout = Timeout::new(25, move || {
                    refresher.emit(());
                });

                timeout.forget();

                true
            }
            Self::Message::Refresh => {
                let upper = &self.refs[0];
                let upper_div = &upper.cast::<HtmlElement>().unwrap();

                console::trace!(
                    "update-history:refresh (T, H)",
                    upper_div.scroll_top(),
                    upper_div.scroll_height()
                );

                upper_div.set_scroll_top(upper_div.scroll_height());

                true
            }
            _ => false,
        }
    }

    fn view(&self, _ctx: &Context<Self>) -> Html {
        html! {
            <div id="hack">
                <div id="upper" ref={&self.refs[0]}>
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
                        <LineEditor onsubmit={self.evaluate_callback.clone()} />
                    </div>
                </div>
            </div>
        }
    }
}

mod internal {
    use crate::text_input::TextInput;
    use gloo_console as console;
    use replies::*;
    use serde::Serialize;
    use std::rc::Rc;
    use yew::prelude::*;
    use yewdux::prelude::*;

    pub type EntryId = u32;

    #[derive(Debug, Serialize, Clone, Eq, PartialEq)]
    pub struct HistoryEntry {
        pub id: EntryId,
        pub value: serde_json::Value,
    }

    impl HistoryEntry {
        pub fn new(value: serde_json::Value) -> Self {
            Self { id: 0, value }
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

    fn simple_entities_list(entities: &Vec<ObservedEntity>) -> Html {
        let names = entities
            .iter()
            .map(
                // TODO Super awkward clone, I'm tired though.
                |e| /* html! { <span> { */ e.name.clone().or(Some("?".into())).unwrap(), /* } </span> } */
            )
            .collect::<Vec<_>>()
            // .intersperse(", "); // TODO This may be a thing some day and
            // would ben ice if this had Html returned above, maybe.
            .join(", ");

        html! {
            <span class="entities">
            { names }
            </span>
        }
    }

    fn area_observation(reply: &AreaObservation) -> Html {
        let name: &str = if let Some(name) = &reply.area.name {
            &name
        } else {
            "No Name"
        };

        let desc: Html = if let Some(desc) = &reply.area.desc {
            html! {
                <p class="desc">{ desc }</p>
            }
        } else {
            html! { <span></span> }
        };

        let living: Html = if reply.living.len() > 0 {
            html! {
                <div class="living">
                    { "Also here is "} { simple_entities_list(&reply.living) } { "." }
                </div>
            }
        } else {
            html! {<span></span>}
        };

        let items: Html = if reply.items.len() > 0 {
            html! {
                <div class="ground">
                    { "You can see "} { simple_entities_list(&reply.items) } { "." }
                </div>
            }
        } else {
            html! {<span></span>}
        };

        let carrying: Html = if reply.carrying.len() > 0 {
            html! {
                <div class="hands">
                    { "You are holding "} { simple_entities_list(&reply.carrying) } { "." }
                </div>
            }
        } else {
            html! {<span></span>}
        };

        let routes: Html = if reply.routes.len() > 0 {
            html! {
                <div class="routes">
                    { "You can see "} { simple_entities_list(&reply.routes) } { "." }
                </div>
            }
        } else {
            html! {<span></span>}
        };

        html! {
            <div class="entry">
                <h3>{ name }</h3>
                { desc }
                { routes }
                { living }
                { items }
                { carrying }
            </div>
        }
    }

    fn inside_observation(reply: &InsideObservation) -> Html {
        html! {
            <div class="living">
                { "Inside is "}{ simple_entities_list(&reply.items) }{ "." }
            </div>
        }
    }

    fn simple_reply(reply: &SimpleReply) -> Html {
        html! {
            <div class="entry">{ format!("{:?}", reply) }</div>
        }
    }

    #[derive(Properties, Clone, PartialEq)]
    struct Props {
        pub entry: HistoryEntry,
    }

    struct HistoryEntryItem {}

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
            let value = &ctx.props().entry.value;

            if let Ok(reply) = serde_json::from_value::<KnownReply>(value.clone()) {
                match reply {
                    KnownReply::AreaObservation(reply) => area_observation(&reply),
                    KnownReply::InsideObservation(reply) => inside_observation(&reply),
                    KnownReply::SimpleReply(reply) => simple_reply(&reply),
                }
            } else {
                html! {
                    <div class="entry">
                        { ctx.props().entry.value.to_string() }
                    </div>
                }
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
