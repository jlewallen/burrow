pub mod model {
    use anyhow::Result;
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;

    use crate::kernel::*;

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Usernames {
        pub users: HashMap<String, EntityKey>,
    }

    impl Needs<std::rc::Rc<dyn Infrastructure>> for Usernames {
        fn supply(&mut self, _infra: &std::rc::Rc<dyn Infrastructure>) -> Result<()> {
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

    impl Default for Usernames {
        fn default() -> Self {
            Self {
                users: Default::default(),
            }
        }
    }
}
