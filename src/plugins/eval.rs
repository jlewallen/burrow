use anyhow::Result;

use crate::kernel::{Action, Entity, EntityKey, EvaluationError};
use crate::plugins;

pub fn evaluate(i: &str) -> Result<Box<dyn Action>, EvaluationError> {
    plugins::looking::evaluate(i)
        .or_else(|_| plugins::carrying::evaluate(i))
        .or_else(|_| plugins::moving::evaluate(i))
        .or_else(|_| plugins::building::evaluate(i))
}

pub fn discover(source: &Entity, entity_keys: &mut Vec<EntityKey>) -> Result<()> {
    plugins::looking::model::discover(source, entity_keys)?;
    plugins::carrying::model::discover(source, entity_keys)?;
    plugins::moving::model::discover(source, entity_keys)?;
    plugins::building::model::discover(source, entity_keys)?;
    Ok(())
}
