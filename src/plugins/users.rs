pub mod model {
    use crate::plugins::library::model::*;

    #[derive(Debug, Serialize, Deserialize, Default)]
    pub struct Usernames {
        pub users: HashMap<String, EntityKey>,
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
}
