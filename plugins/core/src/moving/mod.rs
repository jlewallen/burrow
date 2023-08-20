use crate::library::plugin::*;

pub mod actions;
pub mod model;
mod parser;
#[cfg(test)]
mod tests;

#[derive(Default)]
pub struct MovingPluginFactory {}

impl PluginFactory for MovingPluginFactory {
    fn create_plugin(&self) -> Result<Box<dyn Plugin>> {
        Ok(Box::new(MovingPlugin {}))
    }

    fn stop(&self) -> Result<()> {
        Ok(())
    }
}

#[derive(Default)]
pub struct MovingPlugin {}

impl Plugin for MovingPlugin {
    fn plugin_key() -> &'static str
    where
        Self: Sized,
    {
        "moving"
    }

    fn key(&self) -> &'static str {
        Self::plugin_key()
    }

    fn sources(&self) -> Vec<Box<dyn ActionSource>> {
        vec![Box::new(ActionSources::default())]
    }
}

impl ParsesActions for MovingPlugin {
    fn try_parse_action(&self, i: &str) -> EvaluationResult {
        try_parsing(parser::GoActionParser {}, i)
            .or_else(|_| try_parsing(parser::RouteActionParser {}, i))
    }
}

#[derive(Default)]
pub struct ActionSources {}

impl ActionSource for ActionSources {
    fn try_deserialize_action(
        &self,
        tagged: &TaggedJson,
    ) -> Result<Option<Box<dyn Action>>, serde_json::Error> {
        if let Some(a) = actions::GoAction::from_tagged_json(tagged)? {
            return Ok(Some(Box::new(a)));
        }
        if let Some(a) = actions::AddRouteAction::from_tagged_json(tagged)? {
            return Ok(Some(Box::new(a)));
        }
        if let Some(a) = actions::RemoveRouteAction::from_tagged_json(tagged)? {
            return Ok(Some(Box::new(a)));
        }
        Ok(None)
    }
}
