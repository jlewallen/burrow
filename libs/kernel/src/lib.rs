mod diags;

pub mod actions;
pub mod model;
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
    pub use crate::model::*;
    pub use crate::plugins::*;
    pub use crate::session::*;
    pub use crate::surround::*;

    pub use crate::plugins::{ArgumentType, HasArgumentType};

    pub use crate::diags::{get_diagnostics, Diagnostics};
}

mod finder {
    use crate::model::{Audience, DomainError, EntityKey, EntityPtr, Found, Item};
    use crate::surround::Surroundings;

    pub trait Finder: Send + Sync {
        fn find_world(&self) -> Result<EntityPtr, DomainError>;

        fn find_area(&self, entry: &EntityPtr) -> Result<EntityPtr, DomainError>;

        fn find_item(
            &self,
            surroundings: &Surroundings,
            item: &Item,
        ) -> Result<Option<Found>, DomainError>;

        fn find_audience(&self, audience: &Audience) -> Result<Vec<EntityKey>, DomainError>;
    }
}

#[macro_export]
macro_rules! here {
    () => {{
        format!("{}:{}", file!(), line!())
    }};
}

#[macro_export]
macro_rules! entity_context {
    ($e:expr) => {{
        format!("{}:{} {:?}", file!(), line!(), $e.name().unwrap())
    }};
}

#[cfg(test)]
mod tests;
