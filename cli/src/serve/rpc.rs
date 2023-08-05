use anyhow::Result;
use kernel::Action;
use plugins_rune::actions::SaveScriptAction;
use serde::Deserialize;
use serde_json::Value as JsonValue;

use plugins_core::building::actions::SaveWorkingCopyAction;
use tracing::*;

// Duplicated
#[derive(Deserialize)]
pub enum AcceptableActions {
    SaveWorkingCopyAction(SaveWorkingCopyAction),
    SaveScriptAction(SaveScriptAction),
}

impl AcceptableActions {
    fn into_action(self) -> Box<dyn Action> {
        match self {
            AcceptableActions::SaveWorkingCopyAction(action) => Box::new(action),
            AcceptableActions::SaveScriptAction(action) => Box::new(action),
        }
    }
}

pub fn try_parse_action(value: JsonValue) -> Result<Box<dyn Action>, serde_json::Error> {
    trace!("{:?}", &value);
    serde_json::from_value::<AcceptableActions>(value).map(|a| a.into_action())
}
