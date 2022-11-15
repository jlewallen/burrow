pub mod building;
pub mod carrying;
pub mod eval;
pub mod identifiers;
pub mod library;
pub mod looking;
pub mod moving;
pub mod tools;
pub mod users;

#[cfg(test)]
#[ctor::ctor]
fn initialize_tests() {
    // log_test()
}

#[allow(dead_code)]
fn log_test() {
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "rudder=info,tower_http=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();
}
