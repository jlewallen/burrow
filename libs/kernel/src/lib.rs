pub mod hooks;
pub mod model;
pub mod plugins;
pub mod scopes;
pub mod session;
pub mod surround;

pub use hooks::*;
pub use model::*;
pub use plugins::*;
pub use scopes::*;
pub use session::*;
pub use surround::*;

pub trait Finder: Send + Sync {
    fn find_location(&self, entry: &Entry) -> anyhow::Result<Entry>;

    fn find_item(&self, surroundings: &Surroundings, item: &Item) -> anyhow::Result<Option<Entry>>;

    fn find_audience(&self, audience: &Audience) -> anyhow::Result<Vec<EntityKey>>;
}

pub mod compare {
    use anyhow::Result;
    use thiserror::Error;
    use tracing::*;

    use crate::EntityPtr;

    pub struct AnyChanges<'a> {
        pub entity: &'a EntityPtr,
        pub original: Option<Original<'a>>,
    }

    pub enum Original<'a> {
        String(&'a String),
        Json(&'a serde_json::Value),
    }

    #[derive(Clone, Debug)]
    pub struct Modified {
        pub entity: serde_json::Value,
        pub original: serde_json::Value,
    }

    #[derive(Error, Debug)]
    pub enum CompareError {
        #[error("JSON Error")]
        JsonError(#[source] serde_json::Error),
    }

    impl From<serde_json::Error> for CompareError {
        fn from(source: serde_json::Error) -> Self {
            CompareError::JsonError(source)
        }
    }

    pub fn any_entity_changes(l: AnyChanges) -> Result<Option<Modified>, CompareError> {
        use treediff::diff;
        use treediff::tools::ChangeType;
        use treediff::tools::Recorder;

        let value_after = {
            let entity = l.entity.borrow();

            serde_json::to_value(&*entity)?
        };

        let value_before: serde_json::Value = if let Some(original) = &l.original {
            match original {
                Original::String(s) => s.parse()?,
                Original::Json(v) => (*v).clone(),
            }
        } else {
            serde_json::Value::Null
        };

        let mut d = Recorder::default();
        diff(&value_before, &value_after, &mut d);

        let modifications = d
            .calls
            .iter()
            .filter(|c| !matches!(c, ChangeType::Unchanged(_, _)))
            .count();

        if modifications > 0 {
            for each in d.calls {
                match each {
                    ChangeType::Unchanged(_, _) => {}
                    _ => debug!("modified: {:?}", each),
                }
            }

            Ok(Some(Modified {
                entity: value_after,
                original: value_before,
            }))
        } else {
            Ok(None)
        }
    }
}
