use serde_json::json;
use std::collections::HashMap;

use crate::{
    runner::{Call, RuneReturn, RuneRunner, SharedRunners},
    sources::{get_script, load_sources_from_entity, Relation},
    Behaviors, PerformTagged, ToCall, RUNE_EXTENSION,
};
use plugins_core::library::actions::*;

#[action]
pub struct EditAction {
    pub item: Item,
}

impl Action for EditAction {
    fn is_read_only(&self) -> bool {
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
pub struct DiagnosticsAction {
    pub item: Item,
}

impl Action for DiagnosticsAction {
    fn is_read_only(&self) -> bool
    {
        true
    }

    fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
        match session.find_item(surroundings, &self.item)? {
            Some(editing) => {
                let diagnostics = get_diagnostics(&editing)?;
                Ok(Effect::Reply(EffectReply::TaggedJson(
                    TaggedJson::new_from(json!({ "diagnostics": diagnostics }))?,
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
    fn is_read_only(&self) -> bool {
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
    pub actor: EntityKey,
    pub tagged: TaggedJson,
}

impl Action for RuneAction {
    fn is_read_only(&self) -> bool
    {
        false
    }

    fn perform(&self, session: SessionRef, _surroundings: &Surroundings) -> ReplyResult {
        let Some(actor) = session.entity(&LookupBy::Key(&self.actor))? else {
            return Err(DomainError::EntityNotFound(ErrorContext::Simple(here!())).into());
        };

        let runners = get_local_runners();

        if let Some(call) = self.tagged.to_call() {
            runners.call(call)?.handle(&actor)?;
        }

        Ok(Effect::Ok)
    }
}

#[action]
pub struct RegisterAction {
    pub actor: Item,
}

impl Action for RegisterAction {
    fn is_read_only(&self) -> bool
    {
        false
    }

    fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
        match session.find_item(surroundings, &self.actor)? {
            Some(actor) => {
                let runners = get_local_runners();
                let schema = runners.schema().unwrap();
                if let Some(script) = load_sources_from_entity(&actor, Relation::Actor)? {
                    let mut runner = RuneRunner::new(&schema, script)?;
                    if let Some(post) = runner.call(Call::Register)? {
                        let rr = RuneReturn::new(vec![post.flush()?])?;
                        rr.handle(surroundings.actor())?;
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
