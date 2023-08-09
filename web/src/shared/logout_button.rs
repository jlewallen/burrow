use yew::prelude::*;

use crate::hooks::use_user_context;

#[function_component(LogoutButton)]
pub fn logout_button() -> Html {
    let user_ctx = use_user_context();

    let logout = move |_| {
        user_ctx.logout();
    };

    html! {
        <div class="logout" onclick={logout}>{ "Bye" }</div>
    }
}
