pub mod model {
    use crate::plugins::library::model::*;

    #[derive(Debug, Serialize, Deserialize, Default)]
    pub struct Identifiers {
        pub gid: i64,
        pub acls: Acls,
    }

    impl Needs<std::rc::Rc<dyn Infrastructure>> for Identifiers {
        fn supply(&mut self, _infra: &std::rc::Rc<dyn Infrastructure>) -> Result<()> {
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

    pub fn get_gid(entity: &EntityPtr) -> Result<Option<i64>> {
        let entity = entity.borrow();
        let ids = entity.scope::<Identifiers>()?;

        Ok(Some(ids.gid))
    }

    pub fn set_gid(entity: &EntityPtr, value: i64) -> Result<i64> {
        let mut entity = entity.borrow_mut();
        let mut ids = entity.scope_mut::<Identifiers>()?;
        ids.gid = value;

        Ok(ids.gid)
    }
}
