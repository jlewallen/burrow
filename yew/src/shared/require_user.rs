use yew::prelude::*;
use yew_router::prelude::use_navigator;

use crate::{hooks::use_user_context, routes::Route};

#[derive(Properties, Clone, PartialEq)]
pub struct Props {
    pub children: Children,
}

#[function_component(RequireUser)]
pub fn require_user(props: &Props) -> Html {
    let navigator = use_navigator().unwrap();
    let user_ctx = use_user_context();

    use_effect_with_deps(
        move |(user,)| {
            if !user.is_authenticated() {
                navigator.push(&Route::Login);
            }
        },
        (user_ctx,),
    );

    html! { for props.children.iter() }
}
