use yew::prelude::*;
use yew_router::prelude::*;

use crate::pages::Home;
use crate::pages::Login;
use crate::shared::RequireUser;

#[derive(Debug, Clone, Copy, PartialEq, Routable)]
pub enum Route {
    #[at("/login")]
    Login,
    #[at("/register")]
    Register,
    #[at("/")]
    Home,
    #[not_found]
    #[at("/404")]
    NotFound,
}

pub fn switch(selected_route: Route) -> Html {
    match selected_route {
        Route::Login => html! { <Login /> },
        Route::Register => html! { <Login /> },
        Route::Home => html! { <RequireUser><Home /></RequireUser> },
        Route::NotFound => todo!(),
    }
}
