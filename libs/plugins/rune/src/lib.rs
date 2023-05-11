use glob::glob;
use rune::runtime::RuntimeContext;
use rune::termcolor::{ColorChoice, StandardStream};
use rune::{Context, Diagnostics, Source, Sources, Vm};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use plugins_core::library::plugin::*;
use plugins_core::EntityRelationshipSet;

pub static RUNE_EXTENSION: &str = "rn";

#[derive(Default)]
pub struct RunePluginFactory {}

impl PluginFactory for RunePluginFactory {
    fn create_plugin(&self) -> Result<Box<dyn Plugin>> {
        Ok(Box::new(RunePlugin::default()))
    }
}

pub struct RuneRunner {
    #[allow(dead_code)]
    ctx: Context,
    #[allow(dead_code)]
    runtime: Arc<RuntimeContext>,
    #[allow(dead_code)]
    vm: Option<Vm>,
}

#[derive(PartialEq, Eq, Hash)]
pub enum ScriptSource {
    File(PathBuf),
    Entity(EntityKey, String),
}

pub struct RunePlugin {
    #[allow(dead_code)]
    sources: RefCell<HashSet<ScriptSource>>,
}

impl Default for RunePlugin {
    fn default() -> Self {
        Self {
            sources: RefCell::new(HashSet::new()),
        }
    }
}

impl RunePlugin {
    fn load_user_sources(&self) -> Result<HashSet<ScriptSource>> {
        let mut sources = HashSet::new();
        for entry in glob("user/*.rn")? {
            match entry {
                Ok(path) => {
                    info!("script {}", path.display());
                    sources.insert(ScriptSource::File(path));
                }
                Err(e) => warn!("{:?}", e),
            }
        }

        Ok(sources)
    }

    fn load_sources_from_surroundings(
        &self,
        surroundings: &Surroundings,
    ) -> Result<HashSet<ScriptSource>> {
        let mut sources = HashSet::new();
        let haystack = EntityRelationshipSet::new_from_surroundings(surroundings).expand()?;
        for nearby in haystack
            .iter()
            .map(|r| r.entry())
            .collect::<Result<Vec<_>>>()?
        {
            match get_script(nearby)? {
                Some(script) => {
                    info!("script {:?}", nearby);
                    sources.insert(ScriptSource::Entity(nearby.key().clone(), script));
                }
                None => (),
            }
        }

        Ok(sources)
    }

    fn create_runner(&self, scripts: &HashSet<ScriptSource>) -> Result<RuneRunner> {
        debug!("runner:loading");
        let started = Instant::now();
        let mut sources = Sources::new();
        for script in scripts.iter() {
            match script {
                ScriptSource::File(path) => sources.insert(Source::from_path(path.as_path())?),
                ScriptSource::Entity(key, source) => {
                    sources.insert(Source::new(key.to_string(), source))
                }
            };
        }

        debug!("runner:compiling");
        let mut diagnostics = Diagnostics::new();
        let ctx = Context::with_default_modules()?;
        let runtime: Arc<RuntimeContext> = Default::default();
        let compiled = rune::prepare(&mut sources)
            .with_context(&ctx)
            .with_diagnostics(&mut diagnostics)
            .build();
        if !diagnostics.is_empty() {
            let mut writer = StandardStream::stderr(ColorChoice::Always);
            diagnostics.emit(&mut writer, &sources)?;
        }

        let vm = Vm::new(runtime.clone(), Arc::new(compiled?));
        let elapsed = Instant::now() - started;
        info!("runner:ready {:?}", elapsed);

        Ok(RuneRunner {
            ctx,
            runtime,
            vm: Some(vm),
        })
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
        Ok(())
    }

    fn register_hooks(&self, _hooks: &ManagedHooks) -> Result<()> {
        Ok(())
    }

    fn have_surroundings(&self, surroundings: &Surroundings) -> Result<()> {
        let scripts: HashSet<_> = self
            .load_user_sources()?
            .into_iter()
            .chain(self.load_sources_from_surroundings(surroundings)?)
            .collect();

        match self.create_runner(&scripts) {
            Ok(_runner) => {}
            Err(_e) => warn!("Oops"),
        }

        Ok(())
    }
}

fn get_script(entry: &Entry) -> Result<Option<String>> {
    let behaviors = entry.scope::<Behaviors>()?;
    match &behaviors.langs {
        Some(langs) => match langs.get(RUNE_EXTENSION) {
            Some(script) => Ok(Some(script.clone())),
            None => Ok(None),
        },
        None => Ok(None),
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

    use crate::{get_script, Behaviors, RUNE_EXTENSION};

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
