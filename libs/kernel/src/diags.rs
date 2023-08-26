use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::model::{EntityPtr, JsonValue, OpenScope, Scope};

pub fn get_diagnostics(entity: &EntityPtr) -> anyhow::Result<Option<JsonValue>> {
    if let Some(diagnostics) = entity.scope::<Diagnostics>()? {
        Ok(Some(serde_json::to_value(&*diagnostics)?))
    } else {
        Ok(None)
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Run {
    Diagnostics {
        time: DateTime<Utc>,
        desc: String,
        logs: Vec<serde_json::Value>,
    },
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostics {
    runs: Vec<Run>,
}

impl Scope for Diagnostics {
    fn scope_key() -> &'static str
    where
        Self: Sized,
    {
        "diagnostics"
    }
}

impl Run {
    pub fn new(time: DateTime<Utc>, desc: String, logs: Vec<serde_json::Value>) -> Self {
        Self::Diagnostics { time, desc, logs }
    }
}

impl Diagnostics {
    pub fn new(time: DateTime<Utc>, desc: String, logs: Vec<serde_json::Value>) -> Self {
        Self {
            runs: vec![Run::new(time, desc, logs)],
        }
    }
}
