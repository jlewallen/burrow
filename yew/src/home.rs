use yew::prelude::*;

use gloo_console as console;
use internal::*;
use yew_agent::{
    utils::store::{Bridgeable, ReadOnly, StoreWrapper},
    Bridge,
};

use crate::history::{HistoryStore, PostRequest};

pub struct Home {
    store: Box<dyn Bridge<StoreWrapper<HistoryStore>>>,
}

pub enum Msg {
    HistoryStoreMsg(ReadOnly<HistoryStore>),
    Send(String),
}

impl Component for Home {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let callback = ctx.link().callback(Msg::HistoryStoreMsg);
        Self {
            store: HistoryStore::bridge(callback),
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::HistoryStoreMsg(_state) => {
                // We can see this is logged once before we click any button.
                // The state of the store is sent when we open a bridge.
                console::log!("Received update");

                /*
                let state = state.borrow();
                if state.entries.len() != self.entries.len() {
                    self.entries = state.entries.clone();
                    true
                } else {
                    false
                }
                */
                false
            }
            Msg::Send(text) => {
                self.store.send(PostRequest::Send(text));
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let link = ctx.link();
        html! {
            <div class="home">
                <div><History /></div>
                <div><LineEditor onsubmit={link.callback(|text| Msg::Send(text))} /></div>
            </div>
        }
    }
}

mod internal {
    use crate::history::{HistoryEntry, HistoryStore};
    use crate::text_input::TextInput;
    use gloo_console as console;
    use yew::prelude::*;
    use yew_agent::utils::store::{Bridgeable, ReadOnly, StoreWrapper};
    use yew_agent::Bridge;

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
            // let link = ctx.link();
            html! {
                <div class="entry">
                    { ctx.props().entry.text.as_str() }
                </div>
            }
        }
    }

    pub enum Msg {
        HistoryStoreMsg(ReadOnly<HistoryStore>),
        Send(String),
    }

    #[derive(Properties, Clone, PartialEq)]
    pub struct HistoryProps {}

    pub struct History {
        #[allow(dead_code)]
        store: Box<dyn Bridge<StoreWrapper<HistoryStore>>>,
        entries: Vec<HistoryEntry>,
    }

    impl Component for History {
        type Message = Msg;
        type Properties = HistoryProps;

        fn create(ctx: &Context<Self>) -> Self {
            let callback = ctx.link().callback(Msg::HistoryStoreMsg);
            Self {
                store: HistoryStore::bridge(callback),
                entries: vec![],
            }
        }

        fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
            match msg {
                Msg::HistoryStoreMsg(state) => {
                    // We can see this is logged once before we click any button.
                    // The state of the store is sent when we open a bridge.
                    console::log!("Received update");

                    let state = state.borrow();
                    if state.entries.len() != self.entries.len() {
                        self.entries = state.entries.clone();
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            }
        }

        fn view(&self, _ctx: &Context<Self>) -> Html {
            // let link = ctx.link();
            html! {
                <div class="history">
                    <div class="entries">
                        { for self.entries.iter().map(|entry| html!{ <HistoryEntryItem entry={entry.clone()} /> }) }
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
                _ => false,
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
