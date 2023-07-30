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

    fn middleware(&mut self) -> Result<Vec<Rc<dyn Middleware>>> {
        Ok(vec![Rc::new(RuneMiddleware {
            runners: Arc::clone(&self.runners),
        })])
    }

    fn register_hooks(&self, _hooks: &ManagedHooks) -> Result<()> {
        Ok(())
    }

    fn have_surroundings(&self, surroundings: &Surroundings) -> Result<()> {
        self.add_runners_for(sources::load_sources_from_surroundings(surroundings)?.into_iter())?;

        for (_, runner) in self.runners.borrow_mut().iter_mut() {
            runner.have_surroundings(surroundings)?;
        }

        Ok(())
    }

    fn deliver(&self, incoming: &Incoming) -> Result<()> {
        for (_, runner) in self.runners.borrow_mut().iter_mut() {
            runner.deliver(incoming)?;
        }

        Ok(())
    }

    fn stop(&self) -> Result<()> {
        Ok(())
    }
}

#[derive(Default)]
struct RuneMiddleware {
    runners: Runners,
}

impl RuneMiddleware {}

impl Middleware for RuneMiddleware {
    fn handle(&self, value: Perform, next: MiddlewareNext) -> Result<Effect, anyhow::Error> {
        let _span = span!(Level::INFO, "M", plugin = "rune").entered();

        info!("before");

        let before = {
            let mut runners = self.runners.borrow_mut();

            runners
                .iter_mut()
                .fold(Some(value), |perform, (_, runner)| {
                    perform.and_then(|perform| runner.before(perform).expect("Error in before"))
                })
        };

        if let Some(value) = before {
            let after = next.handle(value)?;

            let mut runners = self.runners.borrow_mut();

            let after = runners.iter_mut().fold(after, |effect, (_, runner)| {
                runner.after(effect).expect("Error in after")
            });

            info!("after");

            Ok(after)
        } else {
            Ok(Effect::Prevented)
        }
    }
}

impl ParsesActions for RunePlugin {
    fn try_parse_action(&self, i: &str) -> EvaluationResult {
        try_parsing(parser::LeadActionParser {}, i)
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
    use plugins_core::library::actions::*;
    use std::collections::HashMap;

    use crate::{sources::get_script, Behaviors, RUNE_EXTENSION};

    #[action]
    pub struct EditAction {
        pub item: Item,
    }

    impl Action for EditAction {
        fn is_read_only() -> bool {
            true
        }

        fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
            info!("editing {:?}", self.item);

            match session.find_item(surroundings, &self.item)? {
                Some(editing) => {
                    let script = match get_script(&editing)? {
                        Some(script) => script,
                        None => "// Default script".to_owned(),
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

    #[action]
    pub struct SaveScriptAction {
        pub key: EntityKey,
        pub copy: WorkingCopy,
    }

    impl Action for SaveScriptAction {
        fn is_read_only() -> bool {
            false
        }

        fn perform(&self, session: SessionRef, _surroundings: &Surroundings) -> ReplyResult {
            info!("saving {:?}", self.key);

            match session.entry(&LookupBy::Key(&self.key))? {
                Some(entry) => {
                    match &self.copy {
                        WorkingCopy::Script(script) => {
                            let mut behaviors = entry.scope_mut::<Behaviors>()?;
                            let langs = behaviors.langs.get_or_insert_with(HashMap::new);
                            langs.insert(RUNE_EXTENSION.to_owned(), script.clone());
                            behaviors.save()?;
                        }
                        _ => unimplemented!(),
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

    use super::actions::EditAction;

    pub struct LeadActionParser {}

    impl ParsesActions for LeadActionParser {
        fn try_parse_action(&self, i: &str) -> EvaluationResult {
            let (_, action) = map(
                preceded(pair(tag("rune"), spaces), noun_or_specific),
                |item| -> EvaluationResult { Ok(Some(Box::new(EditAction { item }))) },
            )(i)?;

            action
        }
    }
}
