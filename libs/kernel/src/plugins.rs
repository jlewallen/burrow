use anyhow::Result;
use std::time::Instant;
use tracing::*;

use super::{model::*, Action, ManagedHooks};
use crate::Surroundings;

pub type EvaluationResult = Result<Box<dyn Action>, EvaluationError>;

pub trait PluginFactory: Send + Sync {
    fn create_plugin(&self) -> Result<Box<dyn Plugin>>;

    fn stop(&self) -> Result<()>;
}

#[derive(Default)]
pub struct RegisteredPlugins {
    factories: Vec<Box<dyn PluginFactory>>,
}

impl RegisteredPlugins {
    pub fn register<P>(&mut self, factory: P)
    where
        P: PluginFactory + 'static,
    {
        self.factories.push(Box::new(factory))
    }

    pub fn create_plugins(&self) -> Result<SessionPlugins> {
        Ok(SessionPlugins::new(
            self.factories
                .iter()
                .map(|f| f.create_plugin())
                .collect::<Result<Vec<_>>>()?,
        ))
    }

    pub fn stop(&self) -> Result<()> {
        for factory in self.factories.iter() {
            factory.stop()?;
        }

        Ok(())
    }
}

pub trait ParsesActions {
    fn try_parse_action(&self, i: &str) -> EvaluationResult;
}

#[derive(Debug)]
pub struct Incoming {
    pub key: String,
    pub serialized: Vec<u8>,
}

impl Incoming {
    pub fn new(key: String, serialized: Vec<u8>) -> Self {
        Self { key, serialized }
    }

    pub fn has_prefix(&self, prefix: &str) -> bool {
        self.key.starts_with(prefix)
    }
}

pub trait Plugin: ParsesActions {
    fn plugin_key() -> &'static str
    where
        Self: Sized;

    fn key(&self) -> &'static str;

    fn initialize(&mut self) -> Result<()>;

    fn register_hooks(&self, hooks: &ManagedHooks) -> Result<()>;

    fn have_surroundings(&self, surroundings: &Surroundings) -> Result<()>;

    fn deliver(&self, incoming: &Incoming) -> Result<()>;

    fn stop(&self) -> Result<()>;
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
            let started = Instant::now();
            plugin.initialize()?;
            let elapsed = Instant::now() - started;
            if elapsed.as_millis() > 20 {
                warn!("plugin:{} ready {:?}", plugin.key(), elapsed);
            } else {
                debug!("plugin:{} ready {:?}", plugin.key(), elapsed);
            }
        }
        Ok(())
    }

    pub fn hooks(&self) -> Result<ManagedHooks> {
        let hooks = ManagedHooks::default();
        for plugin in self.plugins.iter() {
            plugin.register_hooks(&hooks)?;
        }
        Ok(hooks)
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

    pub fn have_surroundings(&self, surroundings: &Surroundings) -> Result<()> {
        for plugin in self.plugins.iter() {
            plugin.have_surroundings(surroundings)?;
        }
        Ok(())
    }

    pub fn deliver(&self, incoming: Incoming) -> Result<()> {
        for plugin in self.plugins.iter() {
            plugin.deliver(&incoming)?;
        }
        Ok(())
    }

    pub fn stop(&self) -> Result<()> {
        for plugin in self.plugins.iter() {
            plugin.stop()?;
        }
        Ok(())
    }
}
