use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use plugins_core::library::plugin::*;

mod runner;
mod sources;

use runner::*;

pub use sources::RUNE_EXTENSION;

#[derive(Default)]
pub struct RunePluginFactory {}

impl PluginFactory for RunePluginFactory {
    fn create_plugin(&self) -> Result<Box<dyn Plugin>> {
        Ok(Box::new(RunePlugin::default()))
    }
}

#[derive(Default)]
pub struct RunePlugin {
    runner: Arc<RefCell<Option<RuneRunner>>>,
}

impl RunePlugin {
    fn use_runner(&self, runner: RuneRunner) {
        self.runner.replace(Some(runner));
    }
}

impl Plugin for RunePlugin {
    fn plugin_key() -> &'static str
    where
        Self: Sized,
    {
        "rune"
    }

    fn initialize(&mut self) -> Result<()> {
        let mut runner = RuneRunner::new(sources::load_user_sources()?)?;

        runner.user()?;

        self.use_runner(runner);

        Ok(())
    }

    fn register_hooks(&self, _hooks: &ManagedHooks) -> Result<()> {
        Ok(())
    }

    fn have_surroundings(&self, surroundings: &Surroundings) -> Result<()> {
        let scripts: HashSet<_> = sources::load_user_sources()?
            .into_iter()
            .chain(sources::load_sources_from_surroundings(surroundings)?)
            .collect();

        let runner = RuneRunner::new(scripts)?;

        runner.have_surroundings(surroundings)?;

        self.use_runner(runner);

        Ok(())
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
    use std::collections::HashMap;

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
                    Ok(Box::new(EditorReply::new(
                        editing.key().to_string(),
                        WorkingCopy::Script(script),
                    )))
                }
                None => Ok(Box::new(SimpleReply::NotFound)),
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
                            let langs = behaviors.langs.get_or_insert_with(|| HashMap::new());
                            langs.insert(RUNE_EXTENSION.to_owned(), script.clone());
                            behaviors.save()?;
                        }
                        _ => {
                            unimplemented!("TODO (See SaveWorkingCopyAction)")
                        }
                    }

                    Ok(Box::new(SimpleReply::Done))
                }
                None => Ok(Box::new(SimpleReply::NotFound)),
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
                |item| -> EvaluationResult { Ok(Box::new(LeadAction { item })) },
            )(i)?;

            action
        }
    }
}
