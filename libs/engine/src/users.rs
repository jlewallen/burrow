pub mod model {
    use anyhow::Result;
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;

    use kernel::prelude::{DomainError, EntityKey, Entry, JsonValue, Scope};

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

    impl Scope for Usernames {
        fn serialize(&self) -> Result<JsonValue> {
            Ok(serde_json::to_value(self)?)
        }

        fn scope_key() -> &'static str {
            "usernames"
        }
    }

    fn username_to_key(world: &Entry, username: &str) -> Result<Option<EntityKey>, DomainError> {
        let usernames = world.scope::<Usernames>()?.expect("No usernames scope");
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
            username_to_key(self, name)
        }

        fn add_username_to_key(&self, username: &str, key: &EntityKey) -> Result<(), DomainError> {
            add_username_to_key(self, username, key)
        }
    }

    const WEB: &str = "web";

    #[derive(Debug, Serialize, Deserialize, Default)]
    pub struct Credentials {
        passwords: HashMap<String, String>,
    }

    impl Credentials {
        pub fn get(&self) -> Option<&String> {
            self.passwords.get(WEB)
        }

        pub fn set(&mut self, secret: String) {
            self.passwords.insert(WEB.to_owned(), secret);
        }
    }

    impl Scope for Credentials {
        fn serialize(&self) -> Result<JsonValue> {
            Ok(serde_json::to_value(self)?)
        }

        fn scope_key() -> &'static str {
            "credentials"
        }
    }

    #[allow(dead_code)]
    const LIMBO: &str = "limbo";
    const ENCYCLOPEDIA: &str = "encyclopedia";
    const WELCOME_AREA: &str = "welcomeArea";

    #[derive(Debug, Serialize, Deserialize, Default)]
    pub struct WellKnown {
        entities: HashMap<String, EntityKey>,
    }

    impl WellKnown {
        pub fn get(&self, name: &str) -> Option<&EntityKey> {
            self.entities.get(name)
        }

        pub fn set(&mut self, name: &str, key: &EntityKey) {
            self.entities.insert(name.to_owned(), key.clone());
        }
    }

    impl Scope for WellKnown {
        fn serialize(&self) -> Result<JsonValue> {
            Ok(serde_json::to_value(self)?)
        }

        fn scope_key() -> &'static str {
            "wellKnown"
        }
    }

    pub trait HasWellKnownEntities {
        fn get_well_known(&self, name: &str) -> Result<Option<EntityKey>, DomainError>;

        fn set_well_known(&self, name: &str, key: &EntityKey) -> Result<(), DomainError>;

        fn get_welcome_area(&self) -> Result<Option<EntityKey>, DomainError> {
            self.get_well_known(WELCOME_AREA)
        }

        fn set_welcome_area(&self, key: &EntityKey) -> Result<(), DomainError> {
            self.set_well_known(WELCOME_AREA, key)
        }

        fn get_encyclopedia(&self) -> Result<Option<EntityKey>, DomainError> {
            self.get_well_known(ENCYCLOPEDIA)
        }

        fn set_encyclopedia(&self, key: &EntityKey) -> Result<(), DomainError> {
            self.set_well_known(ENCYCLOPEDIA, key)
        }
    }

    impl HasWellKnownEntities for Entry {
        fn get_well_known(&self, name: &str) -> Result<Option<EntityKey>, DomainError> {
            let Some(well_known) = self.scope::<WellKnown>()? else {
                return Ok(None);
            };
            Ok(well_known.get(name).cloned())
        }

        fn set_well_known(&self, name: &str, key: &EntityKey) -> Result<(), DomainError> {
            let mut well_known = self.scope_mut::<WellKnown>()?;
            well_known.set(name, key);
            well_known.save()
        }
    }
}
