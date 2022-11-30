pub mod english;
pub mod infra;
pub mod model;
pub mod scopes;

pub use english::*;
pub use infra::*;
pub use model::*;
pub use scopes::*;

pub use replies::*;

pub type ReplyResult = anyhow::Result<Box<dyn Reply>>;
