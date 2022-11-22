use yew::prelude::*;

use internal::*;

pub struct Home {}

pub enum Msg {}

impl Component for Home {
    type Message = Msg;
    type Properties = ();

    fn create(_ctx: &Context<Self>) -> Self {
        Self {}
    }

    fn update(&mut self, _ctx: &Context<Self>, _msg: Self::Message) -> bool {
        false
    }

    fn view(&self, _ctx: &Context<Self>) -> Html {
        html! {
            <div>
                <History />
                <LineEditor />
            </div>
        }
    }
}

mod internal {
    use crate::text_input::TextInput;
    use yew::prelude::*;

    pub enum Msg {
        Send,
    }

    pub struct History {
        value: Vec<serde_json::Value>,
    }

    impl Component for History {
        type Message = Msg;
        type Properties = ();

        fn create(_ctx: &Context<Self>) -> Self {
            Self { value: vec![] }
        }

        fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
            match msg {
                _ => false,
            }
        }

        fn view(&self, _ctx: &Context<Self>) -> Html {
            // let link = ctx.link();
            html! {
                <div>
                </div>
            }
        }
    }

    pub struct LineEditor {}

    impl Component for LineEditor {
        type Message = Msg;
        type Properties = ();

        fn create(_ctx: &Context<Self>) -> Self {
            Self {}
        }

        fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
            match msg {
                Msg::Send => true,
            }
        }

        fn view(&self, ctx: &Context<Self>) -> Html {
            let link = ctx.link();
            html! {
                <div>
                    <TextInput  value="" onsubmit={ctx.link().callback(|_| Msg::Send)} />
                    <button onclick={link.callback(|_| Msg::Send)}>{ "Send" }</button>
                </div>
            }
        }
    }
}
