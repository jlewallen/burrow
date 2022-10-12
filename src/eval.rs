use anyhow::Result;

use super::kernel::{Action, EvaluationError};
use super::plugins;

pub fn evaluate(i: &str) -> Result<Box<dyn Action>, EvaluationError> {
    plugins::looking::evaluate(i)
        .or(plugins::carrying::evaluate(i))
        .or(plugins::moving::evaluate(i))
}
