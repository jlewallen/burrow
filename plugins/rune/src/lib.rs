use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use plugins_core::library::plugin::*;

mod runner;
mod sources;

use runner::*;

pub use sources::{ScriptSource, RUNE_EXTENSION};

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

pub type Runners = Arc<RefCell<HashMap<ScriptSource, RuneRunner>>>;

#[derive(Default)]
pub struct RunePlugin {
    runners: Runners,
}

impl RunePlugin {
    fn add_runners_for(&self, sources: impl Iterator<Item = ScriptSource>) -> Result<()> {
        let mut runners = self.runners.borrow_mut();
        for source in sources {
            if !runners.contains_key(&source) {
                runners.insert(source.clone(), self.create_runner(source)?);
            }
        }

        Ok(())
    }

    fn create_runner(&self, source: ScriptSource) -> Result<RuneRunner> {
        RuneRunner::new(HashSet::from([source]))
    }
}

impl Plugin for RunePlugin {
    fn plugin_key() -> &'static str
    where
        Self: Sized,
    {
        "rune"
    }

    fn key(&self) -> &'static str {
        Self::plugin_key()
    }

    fn initialize(&mut self) -> Result<()> {
        self.add_runners_for(sources::load_user_sources()?.into_iter())?;

        for (_, runner) in self.runners.borrow_mut().iter_mut() {
            runner.user()?;
        }

        Ok(())
    }

    fn register_hooks(&self, hooks: &ManagedHooks) -> Result<()> {
        hooks::register(hooks, &self.runners)
    }

    fn have_surroundings(&self, surroundings: &Surroundings) -> Result<()> {
        self.add_runners_for(sources::load_sources_from_surroundings(surroundings)?.into_iter())?;

        for (_, runner) in self.runners.borrow_mut().iter_mut() {
            runner.have_surroundings(surroundings)?;
        }

        Ok(())
    }

    fn deliver(&self, _incoming: &Incoming) -> Result<()> {
        Ok(())
    }

    fn stop(&self) -> Result<()> {
        Ok(())
    }
}

mod hooks {
    use super::*;
    use plugins_core::moving::model::{AfterMoveHook, BeforeMovingHook, CanMove, MovingHooks};

    pub fn register(hooks: &ManagedHooks, runners: &Runners) -> Result<()> {
        hooks.with::<MovingHooks, _>(|h| {
            let rune_moving_hooks = Box::new(RuneMovingHooks {
                _runners: Runners::clone(runners),
            });
            h.before_moving.register(rune_moving_hooks.clone());
            h.after_move.register(rune_moving_hooks);
            Ok(())
        })
    }

    #[derive(Clone)]
    struct RuneMovingHooks {
        _runners: Runners,
    }

    impl BeforeMovingHook for RuneMovingHooks {
        fn before_moving(&self, _surroundings: &Surroundings, _to_area: &Entry) -> Result<CanMove> {
            Ok(CanMove::Allow)
        }
    }

    impl AfterMoveHook for RuneMovingHooks {
        fn after_move(&self, _surroundings: &Surroundings, _from_area: &Entry) -> Result<()> {
            Ok(())
        }
    }
}

impl ParsesActions for RunePlugin {
    fn try_parse_action(&self, i: &str) -> EvaluationResult {
        try_parsing(parser::LeadActionParser {}, i)
    }
}

impl Evaluator for RunePlugin {
    fn evaluate(&self, perform: &dyn Performer, consider: Evaluation) -> Result<Vec<Effect>> {
        self.evaluate_parsed_action(perform, consider)
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Behaviors {
    pub langs: Option<HashMap<String, String>>,
}

impl Needs<SessionRef> for Behaviors {
    fn supply(&mut self, _session: &SessionRef) -> Result<()> {
        Ok(())
    }
}

impl Scope for Behaviors {
    fn serialize(&self) -> Result<serde_json::Value> {
        Ok(serde_json::to_value(self)?)
    }

    fn scope_key() -> &'static str {
        "behaviors"
    }
}

pub mod actions {
    use std::collections::HashMap;
    use tracing::*;

    use kernel::*;

    use crate::{sources::get_script, Behaviors, RUNE_EXTENSION};

    #[derive(Debug)]
    pub struct LeadAction {
        pub item: Item,
    }

    impl Action for LeadAction {
        fn is_read_only() -> bool {
            true
        }

        fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
            debug!("leading {:?}!", self.item);

            match session.find_item(surroundings, &self.item)? {
                Some(editing) => {
                    info!("leading {:?}", editing);
                    let script = match get_script(&editing)? {
                        Some(script) => script,
                        None => "; Default script".to_owned(),
                    };
                    Ok(
                        EditorReply::new(editing.key().to_string(), WorkingCopy::Script(script))
                            .into(),
                    )
                }
                None => Ok(SimpleReply::NotFound.into()),
            }
        }
    }

    #[derive(Debug)]
    pub struct SaveScriptAction {
        pub key: EntityKey,
        pub copy: WorkingCopy,
    }

    impl Action for SaveScriptAction {
        fn is_read_only() -> bool {
            false
        }

        fn perform(&self, session: SessionRef, _surroundings: &Surroundings) -> ReplyResult {
            info!("mutate:key {:?}", self.key);

            match session.entry(&LookupBy::Key(&self.key))? {
                Some(entry) => {
                    info!("mutate:entry {:?}", entry);

                    match &self.copy {
                        WorkingCopy::Script(script) => {
                            let mut behaviors = entry.scope_mut::<Behaviors>()?;
                            let langs = behaviors.langs.get_or_insert_with(HashMap::new);
                            langs.insert(RUNE_EXTENSION.to_owned(), script.clone());
                            behaviors.save()?;
                        }
                        _ => {
                            unimplemented!("TODO (See SaveWorkingCopyAction)")
                        }
                    }

                    Ok(SimpleReply::Done.into())
                }
                None => Ok(SimpleReply::NotFound.into()),
            }
        }
    }
}

mod parser {
    use kernel::*;
    use plugins_core::library::parser::*;

    use super::actions::LeadAction;

    pub struct LeadActionParser {}

    impl ParsesActions for LeadActionParser {
        fn try_parse_action(&self, i: &str) -> EvaluationResult {
            let (_, action) = map(
                preceded(pair(tag("lead"), spaces), noun_or_specific),
                |item| -> EvaluationResult { Ok(Some(Box::new(LeadAction { item }))) },
            )(i)?;

            action
        }
    }
}
