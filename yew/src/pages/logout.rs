use yew::prelude::*;

use crate::hooks::use_user_context;

#[function_component(Logout)]
pub fn logout_page() -> Html {
    let user_ctx = use_user_context();
    user_ctx.logout();
    html! {
        <div>{{ "Bye!" }}</div>
    }
}
