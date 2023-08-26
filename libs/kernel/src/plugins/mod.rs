use anyhow::Result;
use chrono::{DateTime, Utc};
use replies::{TaggedJson, WorkingCopy};
use serde::Deserialize;
use std::collections::HashMap;
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

#[derive(Debug, Clone)]
pub enum ArgumentType {
    Item,
    String,
    Number,
    Time,
    TaggedJson,
    Optional(Box<ArgumentType>),
}

pub trait HasArgumentType {
    fn argument_type() -> ArgumentType;
}

impl HasArgumentType for Item {
    fn argument_type() -> ArgumentType {
        ArgumentType::Item
    }
}

impl HasArgumentType for EntityKey {
    fn argument_type() -> ArgumentType {
        ArgumentType::Item
    }
}

impl<T: HasArgumentType> HasArgumentType for Option<T> {
    fn argument_type() -> ArgumentType {
        ArgumentType::Optional(T::argument_type().into())
    }
}

impl HasArgumentType for String {
    fn argument_type() -> ArgumentType {
        ArgumentType::String
    }
}

impl HasArgumentType for WorkingCopy {
    fn argument_type() -> ArgumentType {
        ArgumentType::String
    }
}

impl HasArgumentType for DateTime<Utc> {
    fn argument_type() -> ArgumentType {
        ArgumentType::Time
    }
}

impl HasArgumentType for TaggedJson {
    fn argument_type() -> ArgumentType {
        ArgumentType::TaggedJson
    }
}

#[derive(Debug, Clone)]
pub struct ArgSchema {
    name: String,
    ty: ArgumentType,
}

#[derive(Debug, Clone)]
pub struct ActionSchema {
    name: String,
    args: Vec<ArgSchema>,
}

impl ActionSchema {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            args: Vec::new(),
        }
    }

    pub fn arg(mut self, name: &str, ty: ArgumentType) -> Self {
        self.args.push(ArgSchema {
            name: name.to_owned(),
            ty,
        });
        self
    }

    pub fn item(self, name: &str) -> Self {
        self.arg(name, ArgumentType::Item)
    }

    pub fn string(self, name: &str) -> Self {
        self.arg(name, ArgumentType::String)
    }

    pub fn number(self, name: &str) -> Self {
        self.arg(name, ArgumentType::Number)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Schema {
    actions: Vec<ActionSchema>,
}

pub trait HasActionSchema {
    fn action_schema(schema: ActionSchema) -> ActionSchema;
}

impl Schema {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn action<A: Action + HasActionSchema>(mut self) -> Self {
        self.actions
            .push(<A>::action_schema(ActionSchema::new(&<A>::tag())));

        self
    }

    pub fn actions(&self) -> Vec<(String, Vec<(String, ArgumentType)>)> {
        self.actions
            .iter()
            .map(|a| {
                (
                    a.name.to_owned(),
                    a.args
                        .iter()
                        .map(|a| (a.name.to_owned(), a.ty.clone()))
                        .collect(),
                )
            })
            .collect::<Vec<(_, _)>>()
    }
}

#[derive(Debug, Default, Clone)]
pub struct SchemaCollection(HashMap<String, Schema>);

impl SchemaCollection {
    pub fn actions(&self) -> Vec<(String, Vec<(String, Vec<(String, ArgumentType)>)>)> {
        self.0
            .iter()
            .map(|(plugin, schema)| (plugin.to_owned(), schema.actions()))
            .collect::<Vec<_>>()
    }
}

impl From<HashMap<String, Schema>> for SchemaCollection {
    fn from(value: HashMap<String, Schema>) -> Self {
        Self(value)
    }
}

impl Into<HashMap<String, Schema>> for SchemaCollection {
    fn into(self) -> HashMap<String, Schema> {
        self.0
    }
}

pub trait Plugin: ParsesActions {
    fn plugin_key() -> &'static str
    where
        Self: Sized;

    fn key(&self) -> &'static str;

    fn schema(&self) -> Schema {
        Schema::empty()
    }

    fn initialize(&mut self, _schema: &SchemaCollection) -> Result<()> {
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

    pub fn schema(&self) -> SchemaCollection {
        self.plugins
            .iter()
            .map(|p| (p.key().to_owned(), p.schema()))
            .collect::<HashMap<_, _>>()
            .into()
    }

    pub fn initialize(&mut self) -> anyhow::Result<()> {
        let all_schema = self.schema();

        for plugin in self.plugins.iter_mut() {
            let _span = span!(Level::INFO, "I", plugin = plugin.key()).entered();
            let started = Instant::now();
            plugin.initialize(&all_schema)?;
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

#[macro_export]
macro_rules! try_deserialize_all {
    ( $tagged:expr, $( $x:ty ),* ) => {{
        $(
            if let Some(action) = <$x>::from_tagged_json($tagged)? {
                return Ok(Some(Box::new(action)));
            }
        )*
    }};
}

pub trait ActionSource {
    fn try_deserialize_action(
        &self,
        tagged: &TaggedJson,
    ) -> Result<Option<Box<dyn Action>>, serde_json::Error>;
}

impl ActionSource for SessionPlugins {
    fn try_deserialize_action(
        &self,
        tagged: &TaggedJson,
    ) -> Result<Option<Box<dyn Action>>, serde_json::Error> {
        let sources: Vec<_> = self
            .plugins
            .iter()
            .map(|plugin| plugin.sources())
            .flatten()
            .collect();

        Ok(sources
            .iter()
            .map(|source| source.try_deserialize_action(tagged))
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
