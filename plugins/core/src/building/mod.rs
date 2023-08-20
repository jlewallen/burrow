use serde::Deserialize;

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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::enum_variant_names)]
enum SaveActions {
    SaveEntityJsonAction(actions::SaveEntityJsonAction),
    SaveQuickEditAction(actions::SaveQuickEditAction),
}

#[derive(Default)]
pub struct SaveActionSource {}

impl ActionSource for SaveActionSource {
    fn try_deserialize_action(
        &self,
        value: &JsonValue,
    ) -> Result<Option<Box<dyn Action>>, serde_json::Error> {
        type Target = MaybeUnknown<SaveActions, serde_json::Value>;
        serde_json::from_value::<Target>(value.clone()).map(|a| match a {
            MaybeUnknown::Known(SaveActions::SaveEntityJsonAction(action)) => {
                Some(Box::new(action) as Box<dyn Action>)
            }
            MaybeUnknown::Known(SaveActions::SaveQuickEditAction(action)) => {
                Some(Box::new(action) as Box<dyn Action>)
            }
            MaybeUnknown::Unknown(_) => None,
        })
    }
}
