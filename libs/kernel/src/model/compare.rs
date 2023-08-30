use anyhow::Result;
use thiserror::Error;
use tracing::*;

use burrow_bon::prelude::{DottedPath, JsonValue};

pub struct AnyChanges<B, A> {
    pub before: B,
    pub after: A,
}

pub enum Original<'a> {
    String(&'a String),
    Json(&'a JsonValue),
}

#[derive(Clone, Debug)]
pub struct Modified {
    pub paths: Vec<DottedPath>,
    pub before: JsonValue,
    pub after: JsonValue,
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

impl CompareChanges<JsonValue, JsonValue> for TreeDiff {
    fn any_changes(
        &self,
        pair: AnyChanges<JsonValue, JsonValue>,
    ) -> Result<Option<Modified>, CompareError> {
        use treediff::{
            diff,
            tools::{ChangeType, Recorder},
        };

        let mut d = Recorder::default();
        diff(&pair.before, &pair.after, &mut d);

        let calls = d
            .calls
            .iter()
            .filter(|c| !matches!(c, ChangeType::Unchanged(_, _)))
            .collect::<Vec<_>>();

        if !calls.is_empty() {
            let paths = calls
                .iter()
                .flat_map(|c| match c {
                    ChangeType::Removed(k, _)
                    | ChangeType::Added(k, _)
                    | ChangeType::Modified(k, _, _) => k
                        .iter()
                        .map(|k| format!("{}", k))
                        .collect::<Vec<_>>()
                        .into(),
                    ChangeType::Unchanged(_, _) => None,
                })
                .map(|v| v.into())
                .collect::<Vec<DottedPath>>();

            if !paths.is_empty() {
                info!(npaths = %paths.len(), "modifications");
            }

            Ok(Some(Modified {
                paths,
                before: pair.before,
                after: pair.after,
            }))
        } else {
            Ok(None)
        }
    }
}
