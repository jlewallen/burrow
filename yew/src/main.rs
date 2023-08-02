mod app;
mod home;
mod hooks;
mod login;
mod routes;
mod services;

mod command_line;
mod history;
mod list_errors;
mod open_web_socket;
mod text_input;
mod user_context_provider;

mod error {
    use std::collections::HashMap;

    use serde::{Deserialize, Serialize};
    use thiserror::Error as ThisError;

    /// Conduit api error info for Unprocessable Entity error
    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
    #[serde(rename_all = "camelCase")]
    pub struct ErrorInfo {
        pub errors: HashMap<String, Vec<String>>,
    }

    /// Define all possible errors
    #[derive(ThisError, Clone, Debug, PartialEq, Eq)]
    #[allow(dead_code)]
    pub enum Error {
        /// 401
        #[error("Unauthorized")]
        Unauthorized,

        /// 403
        #[error("Forbidden")]
        Forbidden,

        /// 404
        #[error("Not Found")]
        NotFound,

        /// 422
        #[error("Unprocessable Entity: {0:?}")]
        UnprocessableEntity(ErrorInfo),

        /// 500
        #[error("Internal Server Error")]
        InternalServerError,

        /// serde deserialize error
        #[error("Deserialize Error")]
        DeserializeError,

        /// request error
        #[error("Http Request Error")]
        RequestError,
    }
}

mod types {
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Clone, Debug, Default)]
    #[serde(rename_all = "camelCase")]
    pub struct LoginInfo {
        pub email: String,
        pub password: String,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    #[serde(rename_all = "camelCase")]
    pub struct LoginInfoWrapper {
        pub user: LoginInfo,
    }

    #[derive(Serialize, Deserialize, Clone, Debug, Default)]
    #[serde(rename_all = "camelCase")]
    pub struct RegisterInfo {
        pub username: String,
        pub email: String,
        pub password: String,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    #[serde(rename_all = "camelCase")]
    pub struct RegisterInfoWrapper {
        pub user: RegisterInfo,
    }

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
    #[serde(rename_all = "camelCase")]
    pub struct UserInfo {
        pub email: String,
        pub token: String,
        pub username: String,
        pub bio: Option<String>,
        pub image: Option<String>,
    }

    impl UserInfo {
        #[allow(dead_code)]
        pub fn is_authenticated(&self) -> bool {
            !self.token.is_empty()
        }
    }

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
    #[serde(rename_all = "camelCase")]
    pub struct UserInfoWrapper {
        pub user: UserInfo,
    }
}

use app::App;

fn main() {
    wasm_logger::init(wasm_logger::Config::default());
    yew::Renderer::<App>::new().render();
}
