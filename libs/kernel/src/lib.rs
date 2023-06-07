pub mod english;
pub mod hooks;
pub mod model;
pub mod plugins;
pub mod scopes;
pub mod session;
pub mod surround;

pub use english::*;
pub use hooks::*;
pub use model::*;
pub use plugins::*;
pub use scopes::*;
pub use session::*;
pub use surround::*;

pub trait Finder: Send + Sync {
    fn find_location(&self, entry: &Entry) -> anyhow::Result<Entry>;

    fn find_item(&self, surroundings: &Surroundings, item: &Item) -> anyhow::Result<Option<Entry>>;

    fn find_audience(&self, audience: &Audience) -> anyhow::Result<Vec<EntityKey>>;
}
