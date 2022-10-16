pub mod model {
    use crate::kernel::*;
    use anyhow::Result;
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Usernames {
        pub users: HashMap<String, String>,
    }

    impl PrepareWithInfrastructure for Usernames {
        fn prepare_with(&mut self, _infra: &dyn DomainInfrastructure) -> Result<()> {
            Ok(())
        }
    }

    impl Scope for Usernames {
        fn scope_key() -> &'static str {
            "usernames"
        }
    }
}
