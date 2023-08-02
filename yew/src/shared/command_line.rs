use yew::prelude::*;

use crate::shared::TextInput;

#[derive(Properties, Clone, PartialEq)]
pub struct Props {
    pub oncommand: Callback<String>,
}

pub enum Msg {
    Command(String),
}

pub struct CommandLine {}

impl Component for CommandLine {
    type Message = Msg;
    type Properties = Props;

    fn create(_ctx: &Context<Self>) -> Self {
        Self {}
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Command(text) => {
                ctx.props().oncommand.emit(text);
                false
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let link = ctx.link();
        html! {
            <div class="command-line-editor">
                <TextInput value="" onsubmit={link.callback(|text| Msg::Command(text))} />
            </div>
        }
    }
}
