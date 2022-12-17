use anyhow::Result;

use crate::kernel::{Action, EntityKey, Entry, EvaluationError};
use crate::plugins;

use super::library::parser::ParsesActions;

use super::building::BuildingPlugin;
use super::carrying::CarryingPlugin;
use super::looking::LookingPlugin;
use super::moving::MovingPlugin;

pub fn evaluate(i: &str) -> Result<Option<Box<dyn Action>>, EvaluationError> {
    let carrying = CarryingPlugin {};
    let looking = LookingPlugin {};
    let building = BuildingPlugin {};
    let moving = MovingPlugin {};

    match looking
        .try_parse_action(i)
        .or_else(|_| carrying.try_parse_action(i))
        .or_else(|_| moving.try_parse_action(i))
        .or_else(|_| building.try_parse_action(i))
    {
        Ok(e) => Ok(Some(e)),
        Err(_) => Ok(None),
    }
}

pub fn discover(source: &Entry, entity_keys: &mut Vec<EntityKey>) -> Result<()> {
    plugins::looking::model::discover(source, entity_keys)?;
    plugins::carrying::model::discover(source, entity_keys)?;
    plugins::moving::model::discover(source, entity_keys)?;
    plugins::building::model::discover(source, entity_keys)?;
    Ok(())
}
