pub mod model {
    use anyhow::Result;
    use kernel::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, Default)]
    pub struct Identifiers {
        gid: u64,
        acls: Acls,
    }

    impl Needs<SessionRef> for Identifiers {
        fn supply(&mut self, _session: &SessionRef) -> Result<()> {
            Ok(())
        }
    }

    impl Scope for Identifiers {
        fn serialize(&self) -> Result<serde_json::Value> {
            Ok(serde_json::to_value(self)?)
        }

        fn scope_key() -> &'static str {
            "identifiers"
        }
    }

    pub fn fetch_add_one(entity: &Entry) -> Result<EntityGid> {
        let mut ids = entity.scope_mut::<Identifiers>()?;
        ids.gid += 1;
        let value = EntityGid::new(ids.gid);
        ids.save()?;

        Ok(value)
    }
}
