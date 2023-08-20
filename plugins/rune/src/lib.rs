use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::{cell::RefCell, rc::Rc};

use plugins_core::library::plugin::*;
use sources::Script;

mod module;
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

#[derive(Clone, Default)]
pub struct Runners(Arc<RefCell<Vec<RuneRunner>>>);

impl Runners {
    fn add_runners_for(&self, scripts: impl Iterator<Item = Script>) -> Result<()> {
        let mut runners = self.0.borrow_mut();
        for script in scripts {
            runners.push(RuneRunner::new(script)?);
        }

        Ok(())
    }
}

#[derive(Default)]
pub struct RunePlugin {
    runners: Runners,
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

    fn key(&self) -> &'static str {
        Self::plugin_key()
    }

    fn initialize(&mut self) -> Result<()> {
        self.add_runners_for(sources::load_user_sources()?.into_iter())?;

        Ok(())
    }

    fn sources(&self) -> Vec<Box<dyn ActionSource>> {
        vec![Box::new(SaveScriptActionSource::default())]
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
    }
}

#[derive(Default)]
pub struct SaveScriptActionSource {}

impl ActionSource for SaveScriptActionSource {
    fn try_deserialize_action(
        &self,
        tagged: &TaggedJson,
    ) -> Result<Option<Box<dyn Action>>, serde_json::Error> {
        actions::SaveScriptAction::from_tagged_json(tagged)
            .map(|res| res.map(|a| Box::new(a) as Box<dyn Action>))
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

        let handler_rvs: Vec<_> = {
            let mut runners = self.runners.0.borrow_mut();

            runners
                .iter_mut()
                .map(|runner| runner.call_handlers(value.clone()))
                .collect::<Result<Vec<_>>>()?
        };

        let living: Option<EntityPtr> = value.find_living()?;

        if let Some(living) = living {
            for value in handler_rvs.into_iter().flatten() {
                // Annoying that Object doesn't impl Serialize so this clone.
                match value.clone() {
                    rune::Value::Object(object) => {
                        info!("{:#?}", object);
                        let value = serde_json::to_value(value)?;
                        let tagged = TaggedJson::new_from(value)?;

                        let session = get_my_session()?;
                        let action = PerformAction::TaggedJson(tagged);
                        let living = living.clone();
                        session
                            .perform(Perform::Living { living, action })
                            .with_context(|| format!("Rune perform"))?;
                    }
                    rune::Value::Unit => {}
                    _ => warn!("unexpected handler answer: {:#?}", value),
                }
            }
        }

        let before = {
            let mut runners = self.runners.0.borrow_mut();

            runners.iter_mut().fold(Some(value), |perform, runner| {
                perform.and_then(|perform| runner.before(perform).expect("Error in before"))
            })
        };

        if let Some(value) = before {
            let after = next.handle(value)?;

            let mut runners = self.runners.0.borrow_mut();

            let after = runners.iter_mut().fold(after, |effect, runner| {
                runner.after(effect).expect("Error in after")
            });

            info!("after");

            Ok(after)
        } else {
            Ok(Effect::Prevented)
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Behaviors {
    pub langs: Option<HashMap<String, String>>,
}

impl Scope for Behaviors {
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
                    Ok(EditorReply::new(
                        editing.key().to_string(),
                        WorkingCopy::Script(script),
                        SaveScriptAction::new_template(editing.key().clone())?,
                    )
                    .try_into()?)
                }
                None => Ok(SimpleReply::NotFound.try_into()?),
            }
        }
    }

    #[action]
    pub struct SaveScriptAction {
        pub key: EntityKey,
        pub copy: WorkingCopy,
    }

    impl SaveScriptAction {
        pub fn new(key: EntityKey, copy: WorkingCopy) -> Self {
            Self { key, copy }
        }

        pub fn new_template(key: EntityKey) -> Result<JsonTemplate, TaggedJsonError> {
            let copy = WorkingCopy::Script(JSON_TEMPLATE_VALUE_SENTINEL.to_owned());
            let template = Self { key, copy };

            Ok(template.to_tagged_json()?.into())
        }
    }

    impl Action for SaveScriptAction {
        fn is_read_only() -> bool {
            false
        }

        fn perform(&self, session: SessionRef, _surroundings: &Surroundings) -> ReplyResult {
            info!("saving {:?}", self.key);

            match session.entity(&LookupBy::Key(&self.key))? {
                Some(entity) => {
                    match &self.copy {
                        WorkingCopy::Script(script) => {
                            let mut behaviors = entity.scope_mut::<Behaviors>()?;
                            let langs = behaviors.langs.get_or_insert_with(HashMap::new);
                            langs.insert(RUNE_EXTENSION.to_owned(), script.clone());
                            behaviors.save()?;
                        }
                        _ => unimplemented!(),
                    }

                    Ok(SimpleReply::Done.try_into()?)
                }
                None => Ok(SimpleReply::NotFound.try_into()?),
            }
        }
    }
}

mod parser {
    use kernel::prelude::*;
    use plugins_core::library::parser::*;

    use super::actions::EditAction;

    pub struct EditActionParser {}

    impl ParsesActions for EditActionParser {
        fn try_parse_action(&self, i: &str) -> EvaluationResult {
            let (_, action) = map(
                preceded(pair(tag("rune"), spaces), noun_or_specific),
                |item| -> EvaluationResult { Ok(Some(Box::new(EditAction { item }))) },
            )(i)?;

            action
        }
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
            _ => todo!(),
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
