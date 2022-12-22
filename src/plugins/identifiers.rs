pub mod model {
    use crate::plugins::library::model::*;

    #[derive(Debug, Serialize, Deserialize, Default)]
    pub struct Identifiers {
        pub gid: u64,
        pub acls: Acls,
    }

    impl Needs<SessionRef> for Identifiers {
        fn supply(&mut self, _infra: &SessionRef) -> Result<()> {
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

    pub fn get_gid(entity: &Entry) -> Result<Option<EntityGid>> {
        let ids = entity.scope::<Identifiers>()?;

        Ok(Some(EntityGid::new(ids.gid)))
    }

    pub fn set_gid(entity: &Entry, value: EntityGid) -> Result<EntityGid> {
        let mut ids = entity.scope_mut::<Identifiers>()?;
        ids.gid = value.clone().into();

        Ok(value)
    }
}
