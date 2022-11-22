use yew::prelude::*;
use yew_router::prelude::*;

use crate::routes::*;

#[function_component(App)]
pub fn app() -> Html {
    html! {
        <BrowserRouter>
            <div id="app">
                <Switch<Route> render={Switch::render(switch)}/>
            </div>
        </BrowserRouter>
    }
}
