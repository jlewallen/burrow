pub mod build;
pub mod building;
pub mod carrying;
pub mod chat;
pub mod emote;
pub mod fashion;
pub mod finding;
pub mod helping;
pub mod library;
pub mod location;
pub mod looking;
pub mod memory;
pub mod moving;
pub mod security;
pub mod tools;

pub use build::*;
pub use finding::*;

#[cfg(test)]
pub fn log_test() {
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer().with_thread_ids(true))
        .init();
}
