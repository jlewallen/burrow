use crate::library::plugin::*;

#[cfg(test)]
mod tests;

#[derive(Default)]
pub struct ChatPluginFactory {}

impl PluginFactory for ChatPluginFactory {
    fn create_plugin(&self) -> Result<Box<dyn Plugin>> {
        Ok(Box::new(ChatPlugin {}))
    }

    fn stop(&self) -> Result<()> {
        Ok(())
    }
}

#[derive(Default)]
pub struct ChatPlugin {}

impl Plugin for ChatPlugin {
    fn plugin_key() -> &'static str
    where
        Self: Sized,
    {
        "chat"
    }

    fn schema(&self) -> Schema {
        Schema::empty().action::<actions::SpeakAction>()
    }

    fn key(&self) -> &'static str {
        Self::plugin_key()
    }

    fn sources(&self) -> Vec<Box<dyn ActionSource>> {
        vec![Box::new(ActionSources::default())]
    }
}

impl ParsesActions for ChatPlugin {
    fn try_parse_action(&self, i: &str) -> EvaluationResult {
        try_parsing(parser::SpeakActionParser {}, i)
    }
}

#[derive(Default)]
pub struct ActionSources {}

impl ActionSource for ActionSources {
    fn try_deserialize_action(
        &self,
        tagged: &TaggedJson,
    ) -> Result<Option<Box<dyn Action>>, serde_json::Error> {
        try_deserialize_all!(tagged, actions::SpeakAction);

        Ok(None)
    }
}

pub mod model {
    pub use kernel::common::Talking;
}

pub mod actions {
    use anyhow::Context;

    use super::model::*;
    use crate::{library::actions::*, looking::model::Observe};

    #[action]
    pub struct SpeakAction {
        pub(crate) area: Option<Item>,
        pub(crate) speaker: Option<Item>,
        pub(crate) here: Option<String>,
    }

    impl SpeakAction {
        fn say_area(
            &self,
            session: SessionRef,
            speaker: &EntityPtr,
            area: &EntityPtr,
            message: &str,
        ) -> Result<()> {
            Ok(session.raise(
                Some(speaker.clone()),
                Audience::Area(area.key().clone()),
                Raising::TaggedJson(
                    Talking::Conversation(Spoken::new(
                        (&speaker).observe(&speaker)?.expect("No observed entity"),
                        message,
                    ))
                    .to_tagged_json()?,
                ),
            )?)
        }
    }

    impl Action for SpeakAction {
        fn is_read_only() -> bool {
            true
        }

        fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
            if let Some(message) = &self.here {
                let (_, target, _) = surroundings.unpack();

                let speaker = match &self.speaker {
                    Some(speaker) => match session.find_item(&surroundings, &speaker)? {
                        Some(speaker) => speaker,
                        None => return Ok(SimpleReply::NotFound.try_into()?),
                    },
                    None => target,
                };

                let area = match &self.area {
                    Some(area) => match session.find_item(&surroundings, &area)? {
                        Some(area) => area,
                        None => return Ok(SimpleReply::NotFound.try_into()?),
                    },
                    None => tools::area_of(&speaker).with_context(|| "Speaker has no area")?,
                };

                info!(
                    "speaker={:?} area={:?} {:?}",
                    speaker.name()?,
                    area.name()?,
                    &message
                );

                self.say_area(session, &speaker, &area, &message)?;
            }

            Ok(Effect::Ok)
        }
    }
}

pub mod parser {
    use super::actions::*;
    use crate::library::parser::*;

    pub struct SpeakActionParser {}

    impl ParsesActions for SpeakActionParser {
        fn try_parse_action(&self, i: &str) -> EvaluationResult {
            let (_, action) = map(
                preceded(
                    pair(alt((tag("say"), tag("\""))), spaces),
                    text_to_end_of_line,
                ),
                |text| {
                    Box::new(SpeakAction {
                        area: None,
                        speaker: Some(Item::Myself),
                        here: Some(text.to_owned()),
                    }) as Box<dyn Action>
                },
            )(i)?;

            Ok(Some(action))
        }
    }
}
