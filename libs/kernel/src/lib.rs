pub mod actions;
pub mod hooks;
pub mod model;
pub mod perms;
pub mod plugins;
pub mod session;
pub mod surround;

pub mod common {
    pub use replies::*;
}

pub mod prelude {
    pub use replies::DomainEvent;

    pub use crate::here;

    pub use crate::actions::*;
    pub use crate::finder::*;
    pub use crate::hooks::*;
    pub use crate::model::compare::*;
    pub use crate::model::*;
    pub use crate::plugins::*;
    pub use crate::session::*;
    pub use crate::surround::*;
}

mod finder {
    use crate::model::{Audience, EntityKey, EntityPtr, Item};
    use crate::surround::Surroundings;

    pub trait Finder: Send + Sync {
        fn find_world(&self) -> anyhow::Result<EntityPtr>;

        fn find_location(&self, entry: &EntityPtr) -> anyhow::Result<EntityPtr>;

        fn find_item(
            &self,
            surroundings: &Surroundings,
            item: &Item,
        ) -> anyhow::Result<Option<EntityPtr>>;

        fn find_audience(&self, audience: &Audience) -> anyhow::Result<Vec<EntityKey>>;
    }
}

#[macro_export]
macro_rules! here {
    () => {{
        format!("{}:{}", file!(), line!())
    }};
}

#[cfg(test)]
mod tests;
