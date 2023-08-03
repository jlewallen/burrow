use crate::{hooks::*, routes::Route};
use std::ops::Deref;
use yew::prelude::*;
use yew_router::prelude::use_navigator;

#[derive(Properties, Clone, PartialEq)]
pub struct Props {
    pub children: Children,
}

#[function_component(RequireUser)]
pub fn require_user(props: &Props) -> Html {
    let navigator = use_navigator().unwrap();
    let user_ctx = use_user_context();

    use_effect_with_deps(
        move |(user,)| match user.deref() {
            UserContext::Anonymous => {
                navigator.push(&Route::Login);
            }
            _ => {}
        },
        (user_ctx,),
    );

    html! { for props.children.iter() }
}
