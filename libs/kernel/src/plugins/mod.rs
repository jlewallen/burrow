use anyhow::Result;
use serde::Deserialize;
use std::rc::Rc;
use std::time::Instant;
use tracing::*;

use crate::actions::Action;
use crate::model::*;

mod mw;

pub use mw::*;

pub type EvaluationResult = Result<Option<Box<dyn Action>>, EvaluationError>;

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

pub trait Plugin: ParsesActions {
    fn plugin_key() -> &'static str
    where
        Self: Sized;

    fn key(&self) -> &'static str;

    fn initialize(&mut self) -> Result<()> {
        Ok(())
    }

    fn sources(&self) -> Vec<Box<dyn ActionSource>> {
        vec![]
    }

    fn middleware(&mut self) -> Result<Vec<Rc<dyn Middleware>>> {
        Ok(vec![])
    }

    fn stop(&self) -> Result<()> {
        Ok(())
    }
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
            let _span = span!(Level::INFO, "I", plugin = plugin.key()).entered();
            let started = Instant::now();
            plugin.initialize()?;
            let elapsed = Instant::now() - started;
            if elapsed.as_millis() > 200 {
                warn!("plugin:{} ready {:?}", plugin.key(), elapsed);
            } else {
                debug!("plugin:{} ready {:?}", plugin.key(), elapsed);
            }
        }
        Ok(())
    }

    pub fn middleware(&mut self) -> Result<Vec<Rc<dyn Middleware>>> {
        Ok(self
            .plugins
            .iter_mut()
            .map(|plugin| plugin.middleware())
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect())
    }

    pub fn stop(&self) -> Result<()> {
        for plugin in self.plugins.iter() {
            plugin.stop()?;
        }
        Ok(())
    }
}

impl ParsesActions for SessionPlugins {
    fn try_parse_action(&self, i: &str) -> EvaluationResult {
        match self
            .plugins
            .iter()
            .map(|plugin| plugin.try_parse_action(i))
            .filter_map(|r| r.ok())
            .take(1)
            .last()
        {
            Some(Some(e)) => Ok(Some(e)),
            _ => Ok(None),
        }
    }
}

pub trait ActionSource {
    fn try_deserialize_action(
        &self,
        value: &JsonValue,
    ) -> Result<Option<Box<dyn Action>>, serde_json::Error>;
}

impl ActionSource for SessionPlugins {
    fn try_deserialize_action(
        &self,
        value: &JsonValue,
    ) -> Result<Option<Box<dyn Action>>, serde_json::Error> {
        let sources: Vec<_> = self
            .plugins
            .iter()
            .map(|plugin| plugin.sources())
            .flatten()
            .collect();

        Ok(sources
            .iter()
            .map(|source| source.try_deserialize_action(value))
            .collect::<Result<Vec<_>, serde_json::Error>>()?
            .into_iter()
            .flatten()
            .take(1)
            .last())
    }
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum MaybeUnknown<T, U> {
    Known(T),
    Unknown(U),
}
