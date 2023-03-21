use super::{model::*, Action, ManagedHooks};
use anyhow::Result;

pub type EvaluationResult = Result<Box<dyn Action>, EvaluationError>;

pub trait PluginFactory: Send + Sync {
    fn create_plugin(&self) -> Result<Box<dyn Plugin>>;
}

#[derive(Default)]
pub struct RegisteredPlugins {
    factories: Vec<Box<dyn PluginFactory>>,
}

impl RegisteredPlugins {
    pub fn register<P>(&mut self)
    where
        P: PluginFactory + Default + 'static,
    {
        self.factories.push(Box::<P>::default())
    }

    pub fn create_plugins(&self) -> Result<SessionPlugins> {
        Ok(SessionPlugins::new(
            self.factories
                .iter()
                .map(|f| f.create_plugin())
                .collect::<Result<Vec<_>>>()?,
        ))
    }
}

pub trait ParsesActions {
    fn try_parse_action(&self, i: &str) -> EvaluationResult;
}

pub trait Plugin: ParsesActions {
    fn plugin_key() -> &'static str
    where
        Self: Sized;

    fn initialize(&mut self) -> anyhow::Result<()>;

    fn register_hooks(&self, hooks: &ManagedHooks);
}

#[derive(Default)]
pub struct SessionPlugins {
    plugins: Vec<Box<dyn Plugin>>,
}

impl SessionPlugins {
    fn new(plugins: Vec<Box<dyn Plugin>>) -> Self {
        Self { plugins }
    }

    pub fn initialize(&mut self) -> anyhow::Result<()> {
        for plugin in self.plugins.iter_mut() {
            plugin.initialize()?;
        }
        Ok(())
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
