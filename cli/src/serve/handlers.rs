use argon2::{Argon2, PasswordHash, PasswordVerifier};
use axum::{
    extract::Extension,
    http::{header, HeaderMap, Response, StatusCode},
    response::IntoResponse,
    Json,
};
use axum_extra::extract::cookie::{Cookie, SameSite};
use chrono::{DateTime, Utc};
use engine::AfterTick;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{collections::HashMap, ops::Sub, sync::Arc};
use tracing::info;

use super::AppState;

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct LoginUser {
    email: String,
    password: String,
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct LoginUserWrapper {
    user: LoginUser,
}

pub(crate) async fn login_handler(
    Extension(state): Extension<Arc<AppState>>,
    Json(payload): Json<LoginUserWrapper>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    info!("login");

    let user_key = state.find_user_key(&payload.user.email).map_err(|e| {
        let error_response = serde_json::json!({
            "status": "fail",
            "message": format!("Error: {}", e),
        });
        (StatusCode::INTERNAL_SERVER_ERROR, Json(error_response))
    })?;

    let Some(user_key) = user_key else {
            let error_response = serde_json::json!({
                "status": "forbidden"
            });
            return Err((StatusCode::FORBIDDEN, Json(error_response)));
        };
    let Some(hash) = user_key.1 else {
            let error_response = serde_json::json!({
                "status": "forbidden"
            });
            return Err((StatusCode::FORBIDDEN, Json(error_response)));
        };

    let key = user_key.0.to_string();

    let is_valid = match PasswordHash::new(&hash) {
        Ok(parsed_hash) => Argon2::default()
            .verify_password(payload.user.password.as_bytes(), &parsed_hash)
            .map_or(false, |_| true),
        Err(_) => false,
    };

    if !is_valid {
        let error_response = serde_json::json!({
            "status": "fail",
            "message": "Invalid email or password"
        });
        return Err((StatusCode::BAD_REQUEST, Json(error_response)));
    }

    let now = chrono::Utc::now();
    let iat = now.timestamp() as usize;
    let exp = (now + chrono::Duration::hours(72)).timestamp() as usize;
    let claims: TokenClaims = TokenClaims {
        sub: key.to_string(),
        exp,
        iat,
    };

    let token = jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(state.env.jwt_secret.as_ref()),
    )
    .unwrap();

    let cookie = Cookie::build("token", token.to_owned())
        .path("/")
        .max_age(::time::Duration::hours(1))
        .same_site(SameSite::Lax)
        .http_only(true)
        .finish();

    let mut response =
        Response::new(json!({ "user" : { "token": token, "key": key } }).to_string());
    response
        .headers_mut()
        .insert(header::SET_COOKIE, cookie.to_string().parse().unwrap());
    Ok(response)
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct RegisterUser {
    email: String,
    name: String,
    password: String,
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct RegisterUserWrapper {
    user: RegisterUser,
}

pub(crate) async fn register_handler(
    Extension(_state): Extension<Arc<AppState>>,
    Json(_payload): Json<RegisterUserWrapper>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    info!("register");
    Ok(Response::new(
        json!({ "user" : { "token": "".to_owned(), "key": "".to_owned() } }).to_string(),
    ))
}

pub(crate) async fn user_handler(
    Extension(_state): Extension<Arc<AppState>>,
    Extension(user): Extension<User>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    Ok(Response::new(
        json!({ "user" : { "token": user.token, "key": user.key } }).to_string(),
    ))
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct User {
    pub token: String,
    pub key: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenClaims {
    pub sub: String,
    pub iat: usize,
    pub exp: usize,
}

fn empty_map() -> HashMap<String, String> {
    Default::default()
}

fn empty_headers() -> HeaderMap {
    Default::default()
}

fn deadline_headers(now: DateTime<Utc>, deadline: Option<DateTime<Utc>>) -> HeaderMap {
    match deadline {
        Some(deadline) => {
            let mut headers = HeaderMap::new();
            let remaining = deadline.sub(now);
            let remaining = format!("{:?}", remaining.num_milliseconds());
            headers.insert("retry-after", format!("{:?}", deadline).parse().unwrap());
            headers.insert("retry-delay-ms", remaining.parse().unwrap());
            headers
        }
        None => {
            let mut headers = HeaderMap::new();
            let remaining = format!("{}", 1000);
            headers.insert("retry-delay-ms", remaining.parse().unwrap());
            headers
        }
    }
}

pub async fn tick_handler(Extension(state): Extension<Arc<AppState>>) -> impl IntoResponse {
    let now = Utc::now();
    match state.tick(Utc::now()).await {
        Ok(AfterTick::Processed(_)) => {
            info!("tick:processed");

            (StatusCode::OK, empty_headers(), Json(empty_map()))
        }
        Ok(AfterTick::Deadline(deadline)) => {
            info!(%deadline, "tick:too-many");

            (
                StatusCode::TOO_MANY_REQUESTS,
                deadline_headers(now, Some(deadline)),
                Json(empty_map()),
            )
        }
        Ok(AfterTick::Empty) => {
            info!("tick:empty");

            (
                StatusCode::TOO_MANY_REQUESTS,
                deadline_headers(now, None),
                Json(empty_map()),
            )
        }
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            empty_headers(),
            Json(empty_map()),
        ),
    }
}
