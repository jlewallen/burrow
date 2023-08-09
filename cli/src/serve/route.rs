use std::sync::Arc;

use axum::{
    extract::Extension,
    http::{
        header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE},
        HeaderValue, Method,
    },
    middleware,
    routing::{get, get_service, post},
    Router,
};
use std::path::PathBuf;
use tower_http::{
    cors::CorsLayer,
    services::ServeDir,
    trace::{DefaultMakeSpan, TraceLayer},
};

use super::{handlers::*, jwt_auth, state::AppState, ws::*};

pub fn create_router(assets_dir: PathBuf, app_state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin("http://127.0.0.1:8080".parse::<HeaderValue>().unwrap())
        .allow_origin("http://127.0.0.1:5000".parse::<HeaderValue>().unwrap())
        .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE])
        .allow_credentials(true)
        .allow_headers([AUTHORIZATION, ACCEPT, CONTENT_TYPE]);

    Router::new()
        .fallback(get_service(
            ServeDir::new(assets_dir).append_index_html_on_directories(true),
        ))
        .route("/health", get(health_handler))
        .route("/tick", post(tick_handler))
        .route("/ws", get(ws_handler))
        .route(
            "/user",
            get(user_handler).route_layer(middleware::from_fn_with_state(
                app_state.clone(),
                jwt_auth::auth,
            )),
        )
        .route("/user/login", post(login_handler))
        .route("/user", post(register_handler))
        .layer(cors)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::default().include_headers(false)),
        )
        .layer(Extension(app_state))
}
