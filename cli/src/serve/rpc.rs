use anyhow::Result;
use kernel::Action;
use serde_json::Value as JsonValue;

use plugins_core::building::actions::SaveWorkingCopyAction;
use tracing::*;

pub fn try_parse_action(value: JsonValue) -> Result<Box<dyn Action>, serde_json::Error> {
    trace!("{:?}", &value);
    serde_json::from_value::<SaveWorkingCopyAction>(value).map(|a| Box::new(a) as Box<dyn Action>)
}
