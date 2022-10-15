use crate::kernel::EvaluationError;
use crate::kernel::{Action, Entity, EntityKey};
use crate::plugins;
use anyhow::Result;

pub fn evaluate(i: &str) -> Result<Box<dyn Action>, EvaluationError> {
    plugins::looking::evaluate(i)
        .or(plugins::carrying::evaluate(i))
        .or(plugins::moving::evaluate(i))
}

pub fn discover(source: &Entity, entity_keys: &mut Vec<EntityKey>) -> Result<()> {
    plugins::looking::model::discover(source, entity_keys)?;
    plugins::carrying::model::discover(source, entity_keys)?;
    plugins::moving::model::discover(source, entity_keys)?;
    Ok(())
}
