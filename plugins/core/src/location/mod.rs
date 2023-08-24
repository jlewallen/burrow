use crate::library::plugin::*;

use kernel::prelude::EvaluationError;

pub use model::{change_location, container_of, Location};

mod actions;
mod model;
mod parser;
#[cfg(test)]
mod tests;

#[derive(Default)]
pub struct LocationPluginFactory {}

impl PluginFactory for LocationPluginFactory {
    fn create_plugin(&self) -> Result<Box<dyn Plugin>> {
        Ok(Box::new(LocationPlugin {}))
    }

    fn stop(&self) -> Result<()> {
        Ok(())
    }
}

#[derive(Default)]
pub struct LocationPlugin {}

impl Plugin for LocationPlugin {
    fn plugin_key() -> &'static str
    where
        Self: Sized,
    {
        "location"
    }

    fn schema(&self) -> Schema {
        Schema::empty().action::<actions::MoveAction>()
    }

    fn key(&self) -> &'static str {
        Self::plugin_key()
    }

    fn sources(&self) -> Vec<Box<dyn ActionSource>> {
        vec![Box::new(ActionSources::default())]
    }
}

impl ParsesActions for LocationPlugin {
    fn try_parse_action(&self, _i: &str) -> EvaluationResult {
        Err(EvaluationError::ParseFailed)
    }
}

#[derive(Default)]
pub struct ActionSources {}

impl ActionSource for ActionSources {
    fn try_deserialize_action(
        &self,
        tagged: &TaggedJson,
    ) -> Result<Option<Box<dyn Action>>, serde_json::Error> {
        try_deserialize_all!(tagged, actions::MoveAction);

        Ok(None)
    }
}
