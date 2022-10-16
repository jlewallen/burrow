pub mod model {
    use crate::kernel::*;
    use anyhow::Result;
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Usernames {
        pub users: HashMap<String, String>,
    }

    impl LoadReferences for Usernames {
        fn load_refs(&mut self, _infra: &dyn DomainInfrastructure) -> Result<()> {
            Ok(())
        }
    }

    /*
    impl TryFrom<&Entity> for Box<Usernames> {
        type Error = DomainError;

        fn try_from(value: &Entity) -> Result<Self, Self::Error> {
            value.scope::<Usernames>()
        }
    }
    */

    impl Scope for Usernames {
        fn scope_key() -> &'static str {
            "usernames"
        }
    }
}
