use anyhow::Result;

use super::kernel::EntityKey;
use super::kernel::{Action, EvaluationError};
use super::plugins;

pub fn evaluate(i: &str) -> Result<Box<dyn Action>, EvaluationError> {
    plugins::looking::evaluate(i)
        .or(plugins::carrying::evaluate(i))
        .or(plugins::moving::evaluate(i))
}

pub fn discover(entity_keys: &mut Vec<EntityKey>) {
    plugins::looking::model::discover(entity_keys);
    plugins::carrying::model::discover(entity_keys);
    plugins::moving::model::discover(entity_keys);
}
