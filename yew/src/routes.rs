use yew::prelude::*;
use yew_router::prelude::*;

use crate::pages::Example;
use crate::pages::Home;
use crate::pages::Login;
use crate::pages::Register;
use crate::shared::RequireUser;

#[derive(Debug, Clone, Copy, PartialEq, Routable)]
pub enum Route {
    #[at("/login")]
    Login,
    #[at("/register")]
    Register,
    #[at("/")]
    Home,
    #[at("/example")]
    Example,
    #[not_found]
    #[at("/404")]
    NotFound,
}

pub fn switch(selected_route: Route) -> Html {
    match selected_route {
        Route::Login => html! { <Login /> },
        Route::Register => html! { <Register /> },
        Route::Home => html! { <RequireUser><Home /></RequireUser> },
        Route::Example => html! { <RequireUser><Example /></RequireUser> },
        Route::NotFound => todo!(),
    }
}
