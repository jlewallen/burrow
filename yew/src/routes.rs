use yew::prelude::*;
use yew_router::prelude::*;

use crate::pages::Home;
use crate::pages::Login;
use crate::pages::Logout;

#[derive(Debug, Clone, Copy, PartialEq, Routable)]
pub enum Route {
    #[at("/login")]
    Login,
    #[at("/logout")]
    Logout,
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
        Route::Logout => html! { <Logout /> },
        Route::Register => html! { <Login /> },
        Route::Home => html! { <Home /> },
        Route::NotFound => todo!(),
    }
}
