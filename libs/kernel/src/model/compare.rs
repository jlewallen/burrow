use anyhow::Result;
use thiserror::Error;
use tracing::*;

pub struct AnyChanges<B, A> {
    pub before: B,
    pub after: A,
}

pub enum Original<'a> {
    String(&'a String),
    Json(&'a serde_json::Value),
}

#[derive(Clone, Debug)]
pub struct Modified {
    pub before: serde_json::Value,
    pub after: serde_json::Value,
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

pub trait CompareChanges<L, R> {
    fn any_changes(&self, pair: AnyChanges<L, R>) -> Result<Option<Modified>, CompareError>;
}

pub struct TreeDiff {}

impl CompareChanges<serde_json::Value, serde_json::Value> for TreeDiff {
    fn any_changes(
        &self,
        pair: AnyChanges<serde_json::Value, serde_json::Value>,
    ) -> Result<Option<Modified>, CompareError> {
        use treediff::{
            diff,
            tools::{ChangeType, Recorder},
        };

        let mut d = Recorder::default();
        diff(&pair.before, &pair.after, &mut d);

        let modifications = d
            .calls
            .iter()
            .filter(|c| !matches!(c, ChangeType::Unchanged(_, _)))
            .count();

        if modifications > 0 {
            for each in d.calls {
                match each {
                    ChangeType::Unchanged(_, _) => {}
                    ChangeType::Removed(k, _)
                    | ChangeType::Added(k, _)
                    | ChangeType::Modified(k, _, _) => info!(
                        "modified {:?}",
                        k.into_iter()
                            .map(|k| format!("{}", k))
                            .collect::<Vec<_>>()
                            .join(".")
                    ),
                }
            }

            Ok(Some(Modified {
                before: pair.before,
                after: pair.after,
            }))
        } else {
            Ok(None)
        }
    }
}
