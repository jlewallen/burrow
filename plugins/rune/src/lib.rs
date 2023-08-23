use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::{cell::RefCell, rc::Rc};

use rune::Value;

use plugins_core::library::model::DateTime;
use plugins_core::library::plugin::*;
use plugins_core::library::tests::Utc;
use sources::Script;

mod actions;
mod module;
mod parser;
mod runner;
mod sources;

#[cfg(test)]
mod tests;

use runner::*;

pub use sources::{ScriptSource, RUNE_EXTENSION};

use crate::sources::load_sources_from_surroundings;

#[derive(Default)]
pub struct RunePluginFactory {}

impl PluginFactory for RunePluginFactory {
    fn create_plugin(&self) -> Result<Box<dyn Plugin>> {
        Ok(Box::<RunePlugin>::default())
    }

    fn stop(&self) -> Result<()> {
        Ok(())
    }
}

#[derive(Default)]
pub struct RunePlugin {
    runners: SharedRunners,
}

impl RunePlugin {
    fn add_runners_for(&self, scripts: impl Iterator<Item = Script>) -> Result<()> {
        self.runners.add_runners_for(scripts)
    }
}

impl Plugin for RunePlugin {
    fn plugin_key() -> &'static str
    where
        Self: Sized,
    {
        "rune"
    }

    fn schema(&self) -> Schema {
        Schema::empty().action::<actions::RuneAction>()
    }

    fn key(&self) -> &'static str {
        Self::plugin_key()
    }

    fn initialize(&mut self, schema: &SchemaCollection) -> Result<()> {
        self.runners.initialize(schema);

        RUNNERS.with(|setting| {
            let mut setting = setting.borrow_mut();
            *setting = Some(self.runners.weak())
        });

        self.add_runners_for(sources::load_user_sources()?.into_iter())?;

        Ok(())
    }

    fn sources(&self) -> Vec<Box<dyn ActionSource>> {
        vec![Box::new(ActionSources::default())]
    }

    fn middleware(&mut self) -> Result<Vec<Rc<dyn Middleware>>> {
        Ok(vec![Rc::new(RuneMiddleware {
            runners: self.runners.clone(),
        })])
    }
}

impl ParsesActions for RunePlugin {
    fn try_parse_action(&self, i: &str) -> EvaluationResult {
        try_parsing(parser::EditActionParser {}, i)
            .or_else(|_| try_parsing(parser::ShowLogsActionParser {}, i))
    }
}

#[derive(Default)]
pub struct ActionSources {}

impl ActionSource for ActionSources {
    fn try_deserialize_action(
        &self,
        tagged: &TaggedJson,
    ) -> Result<Option<Box<dyn Action>>, serde_json::Error> {
        try_deserialize_all!(tagged, actions::SaveScriptAction, actions::RuneAction);

        Ok(None)
    }
}

#[derive(Default)]
struct RuneMiddleware {
    runners: SharedRunners,
}

impl Middleware for RuneMiddleware {
    fn handle(&self, value: Perform, next: MiddlewareNext) -> Result<Effect, anyhow::Error> {
        let _span = span!(Level::INFO, "M", plugin = "rune").entered();

        info!("before");

        match &value {
            Perform::Surroundings {
                surroundings,
                action: _,
            } => {
                let sources = load_sources_from_surroundings(surroundings)?;
                self.runners.add_runners_for(sources.into_iter())?;
            }
            _ => {}
        }

        if let Some(living) = value.find_living()? {
            if let Some(call) = value.to_call() {
                handle_rune_return(living, self.runners.call(call)?)?;
            }
        }

        let before = self.runners.before(value)?;

        if let Some(value) = before {
            let after = next.handle(value)?;

            let after = self.runners.after(after)?;

            Ok(after)
        } else {
            Ok(Effect::Prevented)
        }
    }
}

pub struct RuneReturn {
    session: SessionRef,
    living: EntityPtr,
    value: rune::runtime::Value,
}

impl RuneReturn {
    fn handle(&self) -> Result<()> {
        // Annoying that Object doesn't impl Serialize so this clone.
        match self.value.clone() {
            rune::Value::Object(_object) => {
                let value = serde_json::to_value(self.value.clone())?;
                info!("{:#?}", &value);

                let tagged = TaggedJson::new_from(value)?;
                let action = PerformAction::TaggedJson(tagged);
                let living = self.living.clone();
                self.session
                    .perform(Perform::Living { living, action })
                    .with_context(|| format!("Rune perform"))?;
            }
            rune::Value::Vec(vec) => {
                let vec = vec.borrow_mut()?;
                for child in vec.iter() {
                    Self {
                        session: self.session.clone(),
                        living: self.living.clone(),
                        value: child.clone(),
                    }
                    .handle()?;
                }
            }
            rune::Value::EmptyTuple => {}
            _ => warn!("unexpected rune return: {:?}", self.value),
        };

        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct LogEntry {
    pub time: DateTime<Utc>,
    pub message: String,
}

impl LogEntry {
    pub fn new_now(message: impl Into<String>) -> Self {
        Self {
            time: Utc::now(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct RuneBehavior {
    pub entry: String,
    pub logs: Vec<LogEntry>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Behaviors {
    pub langs: Option<HashMap<String, RuneBehavior>>,
}

impl Scope for Behaviors {
    fn scope_key() -> &'static str {
        "behaviors"
    }
}

trait TryFindLiving {
    fn find_living(&self) -> Result<Option<EntityPtr>>;
}

impl TryFindLiving for Perform {
    fn find_living(&self) -> Result<Option<EntityPtr>> {
        match self {
            Perform::Surroundings {
                surroundings,
                action: _,
            } => surroundings.find_living(),
            Perform::Raised(raised) => Ok(raised.living.clone()),
            Perform::Schedule(_) => Ok(None),
            _ => todo!("Unable to get living for {:?}", self),
        }
    }
}

impl TryFindLiving for Surroundings {
    fn find_living(&self) -> Result<Option<EntityPtr>> {
        match self {
            Surroundings::Living {
                world: _,
                living,
                area: _,
            } => Ok(Some(living.clone())),
        }
    }
}

trait ToCall {
    fn to_call(&self) -> Option<Call>;
}

impl ToCall for Perform {
    fn to_call(&self) -> Option<Call> {
        match self {
            Perform::Raised(raised) => Some(Call::Handlers(raised.clone())),
            _ => None,
        }
    }
}

impl ToCall for TaggedJson {
    fn to_call(&self) -> Option<Call> {
        Some(Call::Action(self.clone()))
    }
}

fn handle_rune_return(living: EntityPtr, rvs: Vec<Value>) -> Result<()> {
    let session = get_my_session()?;
    for value in rvs.into_iter() {
        RuneReturn {
            session: session.clone(),
            living: living.clone(),
            value,
        }
        .handle()?;
    }

    Ok(())
}

thread_local! {
    static RUNNERS: RefCell<Option<std::sync::Weak<RefCell<Runners>>>> = RefCell::new(None)
}
