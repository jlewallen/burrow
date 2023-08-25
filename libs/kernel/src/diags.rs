use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::model::{EntityPtr, EntityRef, JsonValue, OpenScope, Scope};

pub fn get_diagnostics(entity: &EntityPtr) -> anyhow::Result<Option<JsonValue>> {
    if let Some(diagnostics) = entity.scope::<Diagnostics>()? {
        Ok(Some(serde_json::to_value(&*diagnostics)?))
    } else {
        Ok(None)
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Diagnostics {
    #[default]
    None,
    Foreign(EntityRef),
    Local {
        time: DateTime<Utc>,
        desc: String,
        logs: Vec<serde_json::Value>,
    },
}

impl Scope for Diagnostics {
    fn scope_key() -> &'static str
    where
        Self: Sized,
    {
        "diagnostics"
    }
}

impl Diagnostics {
    pub fn new(time: DateTime<Utc>, desc: String, logs: Vec<serde_json::Value>) -> Self {
        Self::Local { time, desc, logs }
    }
}
