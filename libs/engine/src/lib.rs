mod identifiers;
mod internal;
mod perform;
mod users;

pub mod domain;
pub mod notifications;
pub mod sequences;
pub mod session;

pub mod storage {
    use anyhow::Result;
    use std::rc::Rc;

    use kernel::LookupBy;

    pub trait EntityStorage {
        fn load(&self, lookup: &LookupBy) -> Result<Option<PersistedEntity>>;
        fn save(&self, entity: &PersistedEntity) -> Result<()>;
        fn delete(&self, entity: &PersistedEntity) -> Result<()>;
        fn begin(&self) -> Result<()>;
        fn rollback(&self, benign: bool) -> Result<()>;
        fn commit(&self) -> Result<()>;
        fn query_all(&self) -> Result<Vec<PersistedEntity>>;
    }

    pub trait EntityStorageFactory: Send + Sync {
        fn create_storage(&self) -> Result<Rc<dyn EntityStorage>>;
    }

    #[derive(Debug)]
    pub struct PersistedEntity {
        pub key: String,
        pub gid: u64,
        pub version: u64,
        pub serialized: String,
    }

    impl PersistedEntity {
        pub fn to_json_value(&self) -> Result<serde_json::Value> {
            Ok(serde_json::from_str(&self.serialized)?)
        }
    }
}

pub use domain::*;
pub use notifications::*;
pub use session::*;
pub use storage::*;

pub use users::model::{add_username_to_key, username_to_key};
