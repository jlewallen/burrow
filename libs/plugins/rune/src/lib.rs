use glob::glob;
use rune::runtime::RuntimeContext;
use rune::termcolor::{ColorChoice, StandardStream};
use rune::{Context, Diagnostics, Source, Sources, Vm};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use plugins_core::library::plugin::*;
use plugins_core::EntityRelationshipSet;

#[derive(Default)]
pub struct RunePluginFactory {}

impl PluginFactory for RunePluginFactory {
    fn create_plugin(&self) -> Result<Box<dyn Plugin>> {
        Ok(Box::new(RunePlugin::default()))
    }
}

pub struct RunePlugin {
    ctx: Context,
    runtime: Arc<RuntimeContext>,
    vm: Option<Vm>,
}

impl Default for RunePlugin {
    fn default() -> Self {
        Self {
            ctx: Default::default(),
            runtime: Default::default(),
            vm: None,
        }
    }
}

impl RunePlugin {
    fn load_user_sources(&mut self) -> Result<Sources> {
        let mut sources = Sources::new();
        for entry in glob("user/*.rn")? {
            match entry {
                Ok(path) => {
                    info!("loading {}", path.display());
                    sources.insert(Source::from_path(path.as_path())?);
                }
                Err(e) => warn!("{:?}", e),
            }
        }
        Ok(sources)
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
        let mut sources = self.load_user_sources()?;
        let mut diagnostics = Diagnostics::new();
        let compiled = rune::prepare(&mut sources)
            .with_context(&self.ctx)
            .with_diagnostics(&mut diagnostics)
            .build();

        if !diagnostics.is_empty() {
            let mut writer = StandardStream::stderr(ColorChoice::Always);
            diagnostics.emit(&mut writer, &sources)?;
        }

        let vm = Vm::new(self.runtime.clone(), Arc::new(compiled?));

        self.vm = Some(vm);

        Ok(())
    }

    fn register_hooks(&self, _hooks: &ManagedHooks) -> Result<()> {
        Ok(())
    }

    fn have_surroundings(&self, surroundings: &Surroundings) -> Result<()> {
        let haystack = EntityRelationshipSet::new_from_surroundings(surroundings).expand()?;
        for nearby in haystack
            .iter()
            .map(|r| r.entry())
            .collect::<Result<Vec<_>>>()?
        {
            if nearby.has_scope::<Behaviors>()? {
                let mut behaviors = nearby.scope_mut::<Behaviors>()?;
                if behaviors.langs.is_none() {
                    behaviors.langs = Some(HashMap::new());
                    behaviors.save()?;
                }
                info!("{:?} {:?}", nearby, behaviors.as_ref());
            }
        }

        Ok(())
    }
}

impl ParsesActions for RunePlugin {
    fn try_parse_action(&self, i: &str) -> EvaluationResult {
        try_parsing(parser::LeadActionParser {}, i)
    }
}

pub static RUNE_LANG: &str = "rune";

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

mod actions {
    use kernel::*;

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
                    Ok(Box::new(EditorReply::new(
                        editing.key().to_string(),
                        WorkingCopy::Script(editing.desc()?.unwrap_or("".to_owned())),
                    )))
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
