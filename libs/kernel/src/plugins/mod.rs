use anyhow::Result;
use std::time::Instant;
use tracing::*;

pub use std::rc::Rc;

use super::{model::*, Action, ManagedHooks};
use crate::{Effect, Incoming, Perform, Performer, Surroundings};

pub mod mw;
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

    fn evaluate_parsed_action(
        &self,
        perform: &dyn Performer,
        consider: Evaluable,
    ) -> Result<Vec<Effect>> {
        #[allow(unreachable_patterns)]
        match consider {
            Evaluable::Phrase(text) => self
                .try_parse_action(text)
                .ok()
                .flatten()
                .map(|a| perform.perform(Perform::Chain(a)))
                .map_or(Ok(None), |v| v.map(Some))?
                .map_or(Ok(Vec::new()), |v| Ok(vec![v])),
            _ => todo!(),
        }
    }
}

#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum Evaluable<'a> {
    Phrase(&'a str),
    // Surroundings(Surroundings),
    // Effect(Effect),
}

pub trait Evaluator {
    fn evaluate(&self, performer: &dyn Performer, eval: Evaluable) -> Result<Vec<Effect>>;
}

pub trait Plugin: Evaluator {
    fn plugin_key() -> &'static str
    where
        Self: Sized;

    fn key(&self) -> &'static str;

    fn initialize(&mut self) -> Result<()>;

    fn middleware(&mut self) -> Result<Vec<Rc<dyn Middleware>>>;

    /// Working to remove.
    fn register_hooks(&self, hooks: &ManagedHooks) -> Result<()>;

    /// Working to remove.
    fn have_surroundings(&self, surroundings: &Surroundings) -> Result<()>;

    // If we can get this working alongside Perform and Evaluator we can remove this.
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

    pub fn hooks(&self) -> Result<ManagedHooks> {
        let hooks = ManagedHooks::default();
        for plugin in self.plugins.iter() {
            plugin.register_hooks(&hooks)?;
        }
        Ok(hooks)
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

impl Evaluator for SessionPlugins {
    fn evaluate(&self, performer: &dyn Performer, consider: Evaluable) -> Result<Vec<Effect>> {
        Ok(self
            .plugins
            .iter()
            .map(|plugin| {
                let _span = span!(Level::INFO, "E", plugin = plugin.key()).entered();
                info!("evaluating {:?}", &consider);
                plugin.evaluate(performer, consider.clone())
            })
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect())
    }
}
