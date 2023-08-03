use yew::prelude::*;
use yew_router::prelude::use_navigator;

use crate::routes::Route;

#[function_component(LogoutButton)]
pub fn logout_button() -> Html {
    let navigator = use_navigator().unwrap();

    let logout = move |_| {
        navigator.push(&Route::Login);
    };

    html! {
        <div class="logout" onclick={logout}>{ "Logout" }</div>
    }
}
