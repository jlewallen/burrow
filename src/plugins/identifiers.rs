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

    pub fn get_gid(entity: &EntityPtr) -> Result<Option<EntityGID>> {
        let entity = entity.borrow();
        let ids = entity.scope::<Identifiers>()?;

        Ok(Some(EntityGID::new(ids.gid)))
    }

    pub fn set_gid(entity: &EntityPtr, value: EntityGID) -> Result<EntityGID> {
        let mut entity = entity.borrow_mut();
        let mut ids = entity.scope_mut::<Identifiers>()?;
        ids.gid = value.clone().into();

        Ok(value)
    }
}
