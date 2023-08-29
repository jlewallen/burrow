mod core {
    pub use serde_json::Value as JsonValue;
}

mod perms;

pub mod prelude {
    pub use crate::core::JsonValue;
    pub use crate::perms::*;
}
