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

    fn initialize(&mut self) -> Result<()> {
        Ok(())
    }

    fn middleware(&mut self) -> Result<Vec<Rc<dyn Middleware>>> {
        Ok(Vec::default())
    }

    fn deliver(&self, _incoming: &Incoming) -> Result<()> {
        Ok(())
    }

    fn stop(&self) -> Result<()> {
        Ok(())
    }
}

impl ParsesActions for BuildingPlugin {
    fn try_parse_action(&self, i: &str) -> EvaluationResult {
        try_parsing(parser::EditActionParser {}, i)
            .or_else(|_| try_parsing(parser::DescribeActionParser {}, i))
            .or_else(|_| try_parsing(parser::DuplicateActionParser {}, i))
            .or_else(|_| try_parsing(parser::BidirectionalDigActionParser {}, i))
            .or_else(|_| try_parsing(parser::ObliterateActionParser {}, i))
            .or_else(|_| try_parsing(parser::MakeItemParser {}, i))
    }
}
