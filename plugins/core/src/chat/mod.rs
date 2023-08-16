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

    fn key(&self) -> &'static str {
        Self::plugin_key()
    }
}

impl ParsesActions for ChatPlugin {
    fn try_parse_action(&self, i: &str) -> EvaluationResult {
        try_parsing(parser::SpeakActionParser {}, i)
    }
}

pub mod model {
    pub use kernel::common::Talking;
}

pub mod actions {
    use super::model::*;
    use crate::{library::actions::*, looking::model::Observe};

    #[action]
    pub struct SpeakAction {
        pub(crate) here: Option<String>,
    }

    impl Action for SpeakAction {
        fn is_read_only() -> bool {
            true
        }

        fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
            let (_, living, area) = surroundings.unpack();

            if let Some(message) = &self.here {
                session.raise(
                    Audience::Area(area.key().clone()),
                    Raising::TaggedJson(
                        Talking::Conversation(Spoken::new(
                            (&living).observe(&living)?.expect("No observed entity"),
                            message,
                        ))
                        .to_tagged_json()?,
                    ),
                )?;
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
                        here: Some(text.to_owned()),
                    }) as Box<dyn Action>
                },
            )(i)?;

            Ok(Some(action))
        }
    }
}
