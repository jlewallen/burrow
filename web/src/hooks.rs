use std::fmt;
use std::ops::Deref;

use yew::prelude::*;
use yew_router::prelude::*;

use crate::routes::Route;
use crate::services::set_token;
use crate::types::{Interaction, UserInfo};

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum UserContext {
    Initializing,
    Anonymous,
    User(UserInfo),
}

impl Default for UserContext {
    fn default() -> Self {
        Self::Initializing
    }
}

impl UserContext {
    pub fn key(&self) -> Option<&String> {
        match self {
            UserContext::User(user) => Some(&user.key),
            _ => None,
        }
    }

    pub fn token(&self) -> Option<&String> {
        match self {
            UserContext::User(user) => Some(&user.token),
            _ => None,
        }
    }
}

/// State handle for the [`use_user_context`] hook.
pub struct UseUserContextHandle {
    inner: UseStateHandle<UserContext>,
    navigator: Navigator,
}

impl UseUserContextHandle {
    pub fn login(&self, value: UserInfo) {
        set_token(Some(value.token.clone()));
        self.inner.set(UserContext::User(value));
        self.navigator.push(&Route::Home);
    }

    pub fn logout(&self) {
        set_token(None);
        self.inner.set(UserContext::Anonymous);
        self.navigator.push(&Route::Login);
    }
}

impl Deref for UseUserContextHandle {
    type Target = UserContext;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl Clone for UseUserContextHandle {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            navigator: self.navigator.clone(),
        }
    }
}

impl PartialEq for UseUserContextHandle {
    fn eq(&self, other: &Self) -> bool {
        *self.inner == *other.inner
    }
}

impl fmt::Debug for UseUserContextHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UseUserContextHandle")
            .field("value", &format!("{:?}", *self.inner))
            .finish()
    }
}

/// This hook is used to manage user context.
#[hook]
pub fn use_user_context() -> UseUserContextHandle {
    let inner = use_context::<UseStateHandle<UserContext>>().unwrap();
    let navigator = use_navigator().unwrap();

    UseUserContextHandle { inner, navigator }
}

/// State handle for the [`use_user_interaction`] hook.
pub struct UseUserInteractionHandle {
    inner: UseStateHandle<Interaction>,
}

impl Deref for UseUserInteractionHandle {
    type Target = Interaction;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl Clone for UseUserInteractionHandle {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl PartialEq for UseUserInteractionHandle {
    fn eq(&self, other: &Self) -> bool {
        *self.inner == *other.inner
    }
}

impl fmt::Debug for UseUserInteractionHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UseUserContextHandle")
            .field("value", &format!("{:?}", *self.inner))
            .finish()
    }
}

/// This hook is used to manage user context.
#[hook]
pub fn use_user_interaction() -> UseUserInteractionHandle {
    let inner = use_context::<UseStateHandle<Interaction>>().unwrap();

    UseUserInteractionHandle { inner }
}
