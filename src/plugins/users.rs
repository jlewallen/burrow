    pub mod model {
        use crate::kernel::*;
        use serde::{Deserialize, Serialize};
        use std::collections::HashMap;

        #[derive(Debug, Serialize, Deserialize)]
        pub struct Usernames {
            pub users: HashMap<String, String>,
        }

        impl Scope for Usernames {
            fn scope_key() -> &'static str {
                "usernames"
            }
        }
    }

