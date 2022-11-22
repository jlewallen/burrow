use yew::prelude::*;
use yew_router::prelude::*;

use crate::routes::*;
use crate::text_input::TextInput;

enum Msg {
    Send,
}

struct History {
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

struct LineEditor {}

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
//<History />
//<LineEditor />

#[function_component(App)]
pub fn app() -> Html {
    html! {
        <BrowserRouter>
            <div class="flex w-screen h-screen">
                <Switch<Route> render={Switch::render(switch)}/>
            </div>
        </BrowserRouter>
    }
}
