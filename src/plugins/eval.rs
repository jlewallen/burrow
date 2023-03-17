use anyhow::Result;

use crate::kernel::{Action, EntityKey, Entry, EvaluationError, RegisteredPlugins};
use crate::plugins;

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

pub fn discover(source: &Entry, entity_keys: &mut Vec<EntityKey>) -> Result<()> {
    plugins::looking::model::discover(source, entity_keys)?;
    plugins::carrying::model::discover(source, entity_keys)?;
    plugins::moving::model::discover(source, entity_keys)?;
    plugins::building::model::discover(source, entity_keys)?;
    Ok(())
}
