use crate::library::plugin::*;

pub mod actions;
pub mod model;
pub mod parser;
#[cfg(test)]
mod tests;

#[derive(Default)]
pub struct CarryingPluginFactory {}

impl PluginFactory for CarryingPluginFactory {
    fn create_plugin(&self) -> Result<Box<dyn Plugin>> {
        Ok(Box::new(CarryingPlugin {}))
    }

    fn stop(&self) -> Result<()> {
        Ok(())
    }
}

#[derive(Default)]
pub struct CarryingPlugin {}

impl Plugin for CarryingPlugin {
    fn plugin_key() -> &'static str
    where
        Self: Sized,
    {
        "carrying"
    }

    fn schema(&self) -> Schema {
        Schema::empty()
            .action::<actions::DropAction>()
            .action::<actions::HoldAction>()
            .action::<actions::PutInsideAction>()
            .action::<actions::TakeOutAction>()
            .action::<actions::GiveToAction>()
            .action::<actions::TradeAction>()
    }

    fn key(&self) -> &'static str {
        Self::plugin_key()
    }

    fn sources(&self) -> Vec<Box<dyn ActionSource>> {
        vec![Box::new(ActionSources::default())]
    }
}

impl ParsesActions for CarryingPlugin {
    fn try_parse_action(&self, i: &str) -> EvaluationResult {
        try_parsing(parser::DropActionParser {}, i)
            .or_else(|_| try_parsing(parser::HoldActionParser {}, i))
            .or_else(|_| try_parsing(parser::PutInsideActionParser {}, i))
            .or_else(|_| try_parsing(parser::TakeOutActionParser {}, i))
            .or_else(|_| try_parsing(parser::GiveToActionParser {}, i))
            .or_else(|_| try_parsing(parser::TradeActionParser {}, i))
    }
}

#[derive(Default)]
pub struct ActionSources {}

impl ActionSource for ActionSources {
    fn try_deserialize_action(
        &self,
        tagged: &TaggedJson,
    ) -> Result<Option<Box<dyn Action>>, serde_json::Error> {
        try_deserialize_all!(
            tagged,
            actions::DropAction,
            actions::HoldAction,
            actions::PutInsideAction,
            actions::TakeOutAction,
            actions::GiveToAction
        );

        Ok(None)
    }
}
