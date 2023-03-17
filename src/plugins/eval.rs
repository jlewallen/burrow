use anyhow::Result;

use crate::kernel::{Action, EntityKey, Entry, EvaluationError, Plugin};
use crate::plugins;

use super::building::BuildingPlugin;
use super::carrying::CarryingPlugin;
use super::looking::LookingPlugin;
use super::moving::MovingPlugin;

// TODO These should be registered on the Domain
pub fn registered_plugins() -> Vec<Box<dyn Plugin>> {
    vec![
        Box::new(LookingPlugin {}),
        Box::new(MovingPlugin {}),
        Box::new(CarryingPlugin {}),
        Box::new(BuildingPlugin {}),
    ]
}

pub fn evaluate(i: &str) -> Result<Option<Box<dyn Action>>, EvaluationError> {
    match registered_plugins()
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
