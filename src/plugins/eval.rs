use anyhow::Result;

use crate::kernel::{Action, EvaluationError, RegisteredPlugins};

pub fn evaluate(
    plugins: &RegisteredPlugins,
    i: &str,
) -> Result<Option<Box<dyn Action>>, EvaluationError> {
    match plugins
        .iter()
        .map(|plugin| plugin.try_parse_action(i))
        .filter_map(|r| r.ok())
        .take(1)
        .last()
    {
        Some(e) => Ok(Some(e)),
        None => Ok(None),
    }
}
