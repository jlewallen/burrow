use crate::text_input::TextInput;
// use gloo_console as console;
use yew::prelude::*;

#[derive(Properties, Clone, PartialEq)]
pub struct Props {
    pub onsubmit: Callback<String>,
}

pub enum Msg {
    Send(String),
}

pub struct LineEditor {}

impl Component for LineEditor {
    type Message = Msg;
    type Properties = Props;

    fn create(_ctx: &Context<Self>) -> Self {
        Self {}
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Send(text) => {
                ctx.props().onsubmit.emit(text);
                false
            }
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
