use crate::library::plugin::*;

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

    fn initialize(&mut self) -> Result<()> {
        Ok(())
    }

    fn middleware(&mut self) -> Result<Vec<Rc<dyn Middleware>>> {
        Ok(Vec::default())
    }

    fn deliver(&self, _incoming: &Incoming) -> Result<()> {
        Ok(())
    }

    fn stop(&self) -> Result<()> {
        Ok(())
    }
}

impl ParsesActions for ChatPlugin {
    fn try_parse_action(&self, i: &str) -> EvaluationResult {
        try_parsing(parser::SpeakActionParser {}, i)
    }
}

pub mod model {
    use crate::library::model::*;

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Spoken {
        who: EntityRef,
        message: String,
    }

    impl Spoken {
        pub fn new(who: EntityRef, message: &str) -> Self {
            Self {
                who,
                message: message.to_owned(),
            }
        }
    }

    #[derive(Debug, Serialize, Deserialize, ToJson)]
    #[serde(rename_all = "camelCase")]
    pub enum TalkingEvent {
        Conversation(Spoken),
        Whispering(Spoken),
    }

    impl DomainEvent for TalkingEvent {}
}

pub mod actions {
    use super::model::*;
    use crate::library::actions::*;

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
                    Box::new(TalkingEvent::Conversation(Spoken::new(
                        living.entity_ref(),
                        message,
                    ))),
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

#[cfg(test)]
mod tests {
    // use super::model::*;
    use super::parser::*;
    use crate::library::tests::*;

    #[test]
    fn it_raises_conversation_events() -> Result<()> {
        let mut build = BuildSurroundings::new()?;
        let (session, surroundings) = build.plain().build()?;

        let action = try_parsing(SpeakActionParser {}, "say hello, everyone!")?;
        let action = action.unwrap();
        let reply = action.perform(session.clone(), &surroundings)?;
        let (_, _person, _area) = surroundings.unpack();

        assert!(matches!(reply, Effect::Ok));

        build.close()?;

        Ok(())
    }
}
