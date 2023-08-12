pub mod model {
    use anyhow::Result;
    use serde::{Deserialize, Serialize};

    use kernel::prelude::*;

    #[derive(Debug, Serialize, Deserialize, Default)]
    pub struct Identifiers {
        gid: u64,
        acls: Acls,
    }

    impl Scope for Identifiers {
        fn scope_key() -> &'static str {
            "identifiers"
        }
    }

    pub fn fetch_add_one(entity: &Entry) -> Result<EntityGid> {
        let mut ids = entity.entity().scope_mut::<Identifiers>()?;
        ids.gid += 1;
        let value = EntityGid::new(ids.gid);
        ids.save()?;

        Ok(value)
    }
}
