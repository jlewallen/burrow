use yew::prelude::*;
use yew_router::prelude::*;

use crate::home::Home;
use crate::login::Login;

#[derive(Debug, Clone, Copy, PartialEq, Routable)]
pub enum Route {
    #[at("/login")]
    Login,
    #[at("/")]
    Home,
}

pub fn switch(selected_route: Route) -> Html {
    match selected_route {
        Route::Login => html! {<Login />},
        Route::Home => html! {<Home />},
    }
}
