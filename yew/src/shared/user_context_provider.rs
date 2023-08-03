//! User context provider.

use yew::prelude::*;
use yew_hooks::prelude::*;

use crate::errors::Error;
use crate::hooks::UserContext;
use crate::services::{current, get_token, set_token};

#[derive(Properties, Clone, PartialEq)]
pub struct Props {
    pub children: Children,
}

/// User context provider.
#[function_component(UserContextProvider)]
pub fn user_context_provider(props: &Props) -> Html {
    let user_ctx = use_state(UserContext::default);
    let current_user = use_async(async move { current().await });

    {
        let current_user = current_user.clone();
        use_mount(move || {
            if get_token().is_some() {
                log::info!("user-context: checking token");
                current_user.run();
            } else {
                log::info!("user-context: missing token");
            }
        });
    }

    {
        let user_ctx = user_ctx.clone();
        use_effect_with_deps(
            move |current_user| {
                if let Some(user_info) = &current_user.data {
                    log::info!("user-context: ok!");
                    user_ctx.set(UserContext::User(user_info.user.clone()));
                }

                if let Some(error) = &current_user.error {
                    log::info!("user-context: error {:?}", error);
                    match error {
                        Error::Unauthorized | Error::Forbidden => set_token(None),
                        _ => (),
                    }
                }
                || ()
            },
            current_user,
        )
    }

    html! {
        <ContextProvider<UseStateHandle<UserContext>> context={user_ctx}>
            { for props.children.iter() }
        </ContextProvider<UseStateHandle<UserContext>>>
    }
}
