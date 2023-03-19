use super::{model::*, Action, ManagedHooks};

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

    pub fn hooks(&self) -> ManagedHooks {
        let hooks = ManagedHooks::default();
        for plugin in self.plugins.iter() {
            plugin.register_hooks(&hooks)
        }
        hooks
    }

    pub fn evaluate(&self, i: &str) -> Result<Option<Box<dyn Action>>, EvaluationError> {
        match self
            .plugins
            .iter()
            .map(|plugin| plugin.try_parse_action(i))
            .filter_map(|r| r.ok())
            .take(1)
            .last()
        {
            Some(e) => Ok(Some(e)),
            None => Ok(None),
        }
    }
}
