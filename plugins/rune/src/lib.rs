use anyhow::Context;
use kernel::entity_context;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::{cell::RefCell, rc::Rc};

use plugins_core::library::plugin::*;
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
            .or_else(|_| try_parsing(parser::DiagnosticsActionParser {}, i))
            .or_else(|_| try_parsing(parser::RegisterActionParser {}, i))
    }

    fn try_parse_action_in_surroundings(
        &self,
        surroundings: &Surroundings,
        text: &str,
    ) -> EvaluationResult {
        let sources = load_sources_from_surroundings(surroundings)?;
        self.runners.add_runners_for(sources.into_iter())?;

        match self.try_parse_action(text) {
            Ok(action) => Ok(action),
            Err(_) => {
                let post = self.runners.call(Call::TryParse(text.to_owned()))?;

                warn!("{:?}", post);

                Err(EvaluationError::ParseFailed)
            }
        }
    }
}

#[derive(Default)]
pub struct ActionSources {}

impl ActionSource for ActionSources {
    fn try_deserialize_action(
        &self,
        tagged: &TaggedJson,
    ) -> Result<Option<Box<dyn Action>>, serde_json::Error> {
        try_deserialize_all!(
            tagged,
            actions::SaveScriptAction,
            actions::RuneAction,
            actions::RegisterAction
        );

        Ok(None)
    }
}

#[derive(Default)]
struct RuneMiddleware {
    runners: SharedRunners,
}

impl Middleware for RuneMiddleware {
    fn handle(&self, value: Perform, next: MiddlewareNext) -> Result<Effect, anyhow::Error> {
        let _span = span!(Level::TRACE, "M", plugin = "rune").entered();

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

        if let Some(actor) = value.find_actor()? {
            if let Some(call) = value.to_call() {
                self.runners.call(call)?.handle(&actor)?;
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

pub trait PerformTagged {
    fn handle(&self, actor: &EntityPtr) -> Result<()>;
}

impl PerformTagged for RuneReturn {
    fn handle(&self, actor: &EntityPtr) -> Result<()> {
        for returned in self.simplify().with_context(|| entity_context!(actor))? {
            match returned {
                Returned::Tagged(action) => {
                    let action = PerformAction::TaggedJson(action);
                    let actor = actor.clone();
                    let session = get_my_session()?;
                    session
                        .perform(Perform::Actor { actor, action })
                        .with_context(|| format!("Rune perform"))?;
                }
                _ => {}
            }
        }

        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize, Default, Clone, rune::Any)]
pub struct RuneState {
    value: Option<JsonValue>,
}

impl RuneState {
    #[inline]
    fn string_debug(&self, s: &mut String) -> std::fmt::Result {
        use std::fmt::Write;
        write!(s, "{:?}", self)
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct RuneBehavior {
    pub entry: String,
    pub state: Option<JsonValue>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Behaviors {
    pub langs: Option<HashMap<String, RuneBehavior>>,
}

impl Behaviors {
    pub fn get_rune(&self) -> Option<&RuneBehavior> {
        self.langs.as_ref().map(|f| f.get(RUNE_EXTENSION)).flatten()
    }

    fn get_rune_mut(&mut self) -> Result<&mut RuneBehavior> {
        Ok(self
            .langs
            .get_or_insert_with(|| panic!("Expected langs"))
            .get_mut(RUNE_EXTENSION)
            .expect("Expected rune"))
    }
}

impl Scope for Behaviors {
    fn scope_key() -> &'static str {
        "behaviors"
    }
}

trait TryFindActor {
    fn find_actor(&self) -> Result<Option<EntityPtr>>;
}

impl TryFindActor for Perform {
    fn find_actor(&self) -> Result<Option<EntityPtr>> {
        match self {
            Perform::Surroundings {
                surroundings,
                action: _,
            } => surroundings.find_actor(),
            Perform::Raised(raised) => Ok(raised.actor.clone()),
            Perform::Schedule(_) => Ok(None),
            _ => todo!("Unable to get actor for {:?}", self),
        }
    }
}

impl TryFindActor for Surroundings {
    fn find_actor(&self) -> Result<Option<EntityPtr>> {
        match self {
            Surroundings::Actor {
                world: _,
                actor,
                area: _,
            } => Ok(Some(actor.clone())),
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

thread_local! {
    static RUNNERS: RefCell<Option<std::sync::Weak<RefCell<Runners>>>> = RefCell::new(None)
}
