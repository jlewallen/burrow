pub mod model {
    use crate::kernel::*;
    use anyhow::Result;
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;

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
        fn scope_key() -> &'static str {
            "usernames"
        }
    }
}
