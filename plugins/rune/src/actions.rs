use serde_json::json;
use std::collections::HashMap;

use crate::{
    runner::{Call, RuneRunner, SharedRunners},
    sources::{get_logs, get_script, load_sources_from_entity, Relation},
    Behaviors, PerformTagged, ToCall, RUNE_EXTENSION,
};
use plugins_core::library::actions::*;

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
                    Some(script) => script.entry().to_owned(),
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
pub struct ShowLogAction {
    pub item: Item,
}

impl Action for ShowLogAction {
    fn is_read_only() -> bool
    where
        Self: Sized,
    {
        true
    }

    fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
        info!("editing {:?}", self.item);

        match session.find_item(surroundings, &self.item)? {
            Some(editing) => {
                let logs = match get_logs(&editing)? {
                    Some(logs) => logs,
                    None => Vec::default(),
                };
                let logs = serde_json::to_value(logs)?;
                Ok(Effect::Reply(EffectReply::TaggedJson(
                    TaggedJson::new_from(json!({ "logs": logs }))?,
                )))
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
                        let ours = langs.entry(RUNE_EXTENSION.to_owned()).or_default();
                        ours.entry = script.clone();
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

fn get_local_runners() -> SharedRunners {
    super::RUNNERS.with(|getting| {
        let getting = getting.borrow();
        if let Some(weak) = &*getting {
            if let Some(runners) = weak.upgrade() {
                return SharedRunners::new(runners.clone());
            }
        }

        panic!();
    })
}

#[action]
pub struct RuneAction {
    pub target: EntityKey,
    pub tagged: TaggedJson,
}

impl Action for RuneAction {
    fn is_read_only() -> bool
    where
        Self: Sized,
    {
        false
    }

    fn perform(&self, session: SessionRef, _surroundings: &Surroundings) -> ReplyResult {
        let Some(target) = session.entity(&LookupBy::Key(&self.target))? else {
            return Err(DomainError::EntityNotFound(ErrorContext::Simple(here!())).into());
        };

        let runners = get_local_runners();

        if let Some(call) = self.tagged.to_call() {
            runners.call(call)?.handle(target)?;
        }

        Ok(Effect::Ok)
    }
}

#[action]
pub struct RegisterAction {
    pub target: Item,
}

impl Action for RegisterAction {
    fn is_read_only() -> bool
    where
        Self: Sized,
    {
        false
    }

    fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
        match session.find_item(surroundings, &self.target)? {
            Some(target) => {
                let runners = get_local_runners();
                let schema = runners.schema().unwrap();
                if let Some(script) = load_sources_from_entity(&target, Relation::Target)? {
                    let mut runner = RuneRunner::new(&schema, script)?;
                    if let Some(post) = runner.call(Call::Register)? {
                        post.flush()?;
                    }

                    Ok(SimpleReply::Done.try_into()?)
                } else {
                    Ok(SimpleReply::Impossible.try_into()?)
                }
            }
            None => Ok(SimpleReply::NotFound.try_into()?),
        }
    }
}
