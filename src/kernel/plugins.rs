use crate::domain::ManagedHooks;

use super::{model::*, Action};

pub type EvaluationResult = Result<Box<dyn Action>, EvaluationError>;

pub trait ParsesActions {
    fn try_parse_action(&self, i: &str) -> EvaluationResult;
}

pub trait Plugin: ParsesActions + Send + Sync {
    fn plugin_key() -> &'static str
    where
        Self: Sized;

    fn register_hooks(&self, hooks: &ManagedHooks);
}

#[derive(Default)]
pub struct RegisteredPlugins {
    plugins: Vec<Box<dyn Plugin>>,
}

impl RegisteredPlugins {
    pub fn register<P>(&mut self)
    where
        P: Plugin + Default + 'static,
    {
        self.plugins.push(Box::<P>::default())
    }

    pub fn hooks(self: &Self) -> ManagedHooks {
        let hooks = ManagedHooks::default();
        for plugin in self.plugins.iter() {
            plugin.register_hooks(&hooks)
        }
        hooks
    }

    pub fn iter(self: &Self) -> impl Iterator<Item = &Box<dyn Plugin>> {
        self.plugins.iter()
    }
}
