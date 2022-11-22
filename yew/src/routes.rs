// use wasm_bindgen::prelude::*;
// use yew::functional::*;
use yew::prelude::*;
use yew_router::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Routable)]
pub enum Route {
    #[at("/login")]
    Login,
    #[at("/")]
    Home,
}

pub fn switch(selected_route: &Route) -> Html {
    match selected_route {
        Route::Login => html! {<h1>{ "Login" }</h1>},
        Route::Home => html! {<h1>{ "Home" }</h1>},
    }
}
