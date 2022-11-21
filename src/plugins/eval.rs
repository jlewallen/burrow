use anyhow::Result;

use crate::kernel::{Action, Entity, EntityKey, EvaluationError};
use crate::plugins;

use super::building::actions::BuildingPlugin;
use super::carrying::actions::CarryingPlugin;
use super::library::actions::ParsesActions;
use super::looking::actions::LookingPlugin;
use super::moving::actions::MovingPlugin;

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

pub fn discover(source: &Entity, entity_keys: &mut Vec<EntityKey>) -> Result<()> {
    plugins::looking::model::discover(source, entity_keys)?;
    plugins::carrying::model::discover(source, entity_keys)?;
    plugins::moving::model::discover(source, entity_keys)?;
    plugins::building::model::discover(source, entity_keys)?;
    Ok(())
}
