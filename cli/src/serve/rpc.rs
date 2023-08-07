use anyhow::Result;
use serde::Deserialize;
use serde_json::Value as JsonValue;
use tracing::*;

use kernel::Action;
use plugins_core::{
    building::actions::{SaveEntityJsonAction, SaveQuickEditAction},
    helping::actions::SaveHelpAction,
};
use plugins_rune::actions::SaveScriptAction;

// Duplicated
#[derive(Deserialize)]
pub enum AcceptableActions {
    SaveEntityJsonAction(SaveEntityJsonAction),
    SaveQuickEditAction(SaveQuickEditAction),
    SaveScriptAction(SaveScriptAction),
    SaveHelpAction(SaveHelpAction),
}

impl AcceptableActions {
    fn into_action(self) -> Box<dyn Action> {
        match self {
            AcceptableActions::SaveEntityJsonAction(action) => Box::new(action),
            AcceptableActions::SaveQuickEditAction(action) => Box::new(action),
            AcceptableActions::SaveScriptAction(action) => Box::new(action),
            AcceptableActions::SaveHelpAction(action) => Box::new(action),
        }
    }
}

pub fn try_parse_action(value: JsonValue) -> Result<Box<dyn Action>, serde_json::Error> {
    trace!("{:?}", &value);
    serde_json::from_value::<AcceptableActions>(value).map(|a| a.into_action())
}
