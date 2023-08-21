use crate::library::plugin::*;

pub mod actions;
pub mod model;
pub mod parser;
#[cfg(test)]
mod tests;

#[derive(Default)]
pub struct BuildingPluginFactory {}

impl PluginFactory for BuildingPluginFactory {
    fn create_plugin(&self) -> Result<Box<dyn Plugin>> {
        Ok(Box::new(BuildingPlugin {}))
    }

    fn stop(&self) -> Result<()> {
        Ok(())
    }
}

#[derive(Default)]
pub struct BuildingPlugin {}

impl Plugin for BuildingPlugin {
    fn plugin_key() -> &'static str
    where
        Self: Sized,
    {
        "building"
    }

    fn key(&self) -> &'static str {
        Self::plugin_key()
    }

    fn schema(&self) -> Schema {
        Schema::empty()
            .action::<actions::EditAction>()
            .action::<actions::DuplicateAction>()
            .action::<actions::BidirectionalDigAction>()
            .action::<actions::ObliterateAction>()
            .action::<actions::MakeItemAction>()
            .action::<actions::BuildAreaAction>()
            .action::<actions::AddScopeAction>()
    }

    fn sources(&self) -> Vec<Box<dyn ActionSource>> {
        vec![Box::new(SaveActionSource::default())]
    }
}

impl ParsesActions for BuildingPlugin {
    fn try_parse_action(&self, i: &str) -> EvaluationResult {
        try_parsing(parser::EditActionParser {}, i)
            .or_else(|_| try_parsing(parser::DuplicateActionParser {}, i))
            .or_else(|_| try_parsing(parser::BidirectionalDigActionParser {}, i))
            .or_else(|_| try_parsing(parser::ObliterateActionParser {}, i))
            .or_else(|_| try_parsing(parser::MakeItemParser {}, i))
            .or_else(|_| try_parsing(parser::BuildAreaParser {}, i))
            .or_else(|_| try_parsing(parser::ScopeActionParser {}, i))
    }
}

#[derive(Default)]
pub struct SaveActionSource {}

impl ActionSource for SaveActionSource {
    fn try_deserialize_action(
        &self,
        tagged: &TaggedJson,
    ) -> Result<Option<Box<dyn Action>>, serde_json::Error> {
        try_deserialize_all!(
            tagged,
            actions::SaveQuickEditAction,
            actions::SaveEntityJsonAction,
            actions::DuplicateAction,
            actions::MakeItemAction,
            actions::BuildAreaAction,
            actions::AddScopeAction,
            actions::ObliterateAction
        );

        Ok(None)
    }
}
