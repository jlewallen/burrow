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

        pub fn set(&mut self, name: &str, key: &EntityKey) {
            self.users.insert(name.to_owned(), key.clone());
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

    fn username_to_key(world: &Entry, username: &str) -> Result<Option<EntityKey>, DomainError> {
        let usernames = world.scope::<Usernames>()?;
        Ok(usernames.find(username).cloned())
    }

    fn add_username_to_key(
        world: &Entry,
        username: &str,
        key: &EntityKey,
    ) -> Result<(), DomainError> {
        let mut usernames = world.scope_mut::<Usernames>()?;
        usernames.set(username, key);
        usernames.save()
    }

    pub trait HasUsernames {
        fn find_name_key(&self, name: &str) -> Result<Option<EntityKey>, DomainError>;
        fn add_username_to_key(&self, username: &str, key: &EntityKey) -> Result<(), DomainError>;
    }

    impl HasUsernames for Entry {
        fn find_name_key(&self, name: &str) -> Result<Option<EntityKey>, DomainError> {
            let _span = tracing::span!(tracing::Level::DEBUG, "who").entered();

            username_to_key(self, name)
        }

        fn add_username_to_key(&self, username: &str, key: &EntityKey) -> Result<(), DomainError> {
            add_username_to_key(self, username, key)
        }
    }
}
