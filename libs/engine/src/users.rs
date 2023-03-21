pub mod model {
    use anyhow::Result;
    use kernel::*;
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;

    #[derive(Debug, Serialize, Deserialize, Default)]
    pub struct Usernames {
        users: HashMap<String, EntityKey>,
    }

    impl Usernames {
        pub fn find(&self, name: &str) -> Option<&EntityKey> {
            self.users.get(name)
        }
    }

    impl Needs<SessionRef> for Usernames {
        fn supply(&mut self, _session: &SessionRef) -> Result<()> {
            Ok(())
        }
    }

    impl Scope for Usernames {
        fn serialize(&self) -> Result<serde_json::Value> {
            Ok(serde_json::to_value(self)?)
        }

        fn scope_key() -> &'static str {
            "usernames"
        }
    }

    pub fn username_to_key(world: &Entry, username: &str) -> Result<Option<EntityKey>> {
        let usernames = world.scope::<Usernames>()?;
        Ok(usernames.find(username).cloned())
    }
}
