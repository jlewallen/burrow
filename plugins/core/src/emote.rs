use crate::library::plugin::*;

#[derive(Default)]
pub struct EmotePluginFactory {}

impl PluginFactory for EmotePluginFactory {
    fn create_plugin(&self) -> Result<Box<dyn Plugin>> {
        Ok(Box::new(EmotePlugin {}))
    }

    fn stop(&self) -> Result<()> {
        Ok(())
    }
}

#[derive(Default)]
pub struct EmotePlugin {}

impl Plugin for EmotePlugin {
    fn plugin_key() -> &'static str
    where
        Self: Sized,
    {
        "emote"
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

impl ParsesActions for EmotePlugin {
    fn try_parse_action(&self, i: &str) -> EvaluationResult {
        try_parsing(parser::LaughActionParser {}, i)
    }
}

pub mod model {
    use crate::library::model::*;

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Emoted {
        pub who: EntityRef,
    }

    impl Emoted {
        pub fn new(who: EntityRef) -> Self {
            Self { who }
        }
    }

    #[derive(Debug, Serialize, Deserialize, ToJson)]
    #[serde(rename_all = "camelCase")]
    pub enum EmotingEvent {
        Laugh(Emoted),
    }

    impl DomainEvent for EmotingEvent {}
}

pub mod actions {
    use super::model::*;
    use crate::library::actions::*;

    #[action]
    pub struct LaughAction {}

    impl Action for LaughAction {
        fn is_read_only() -> bool {
            true
        }

        fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
            let (_, living, area) = surroundings.unpack();

            session.raise(
                Audience::Area(area.key().clone()),
                Box::new(EmotingEvent::Laugh(Emoted::new(living.entity_ref()))),
            )?;

            Ok(Effect::Ok)
        }
    }
}

pub mod parser {
    use super::actions::*;
    use crate::library::parser::*;

    pub struct LaughActionParser {}

    impl ParsesActions for LaughActionParser {
        fn try_parse_action(&self, i: &str) -> EvaluationResult {
            let (_, action) = map(alt((tag("laugh"), tag("lol"))), |_s| {
                Box::new(LaughAction {}) as Box<dyn Action>
            })(i)?;

            Ok(Some(action))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parser::*;
    use crate::library::tests::*;

    #[test]
    fn it_raises_laugh_events() -> Result<()> {
        let mut build = BuildSurroundings::new()?;
        let (session, surroundings) = build.plain().build()?;

        let action = try_parsing(LaughActionParser {}, "laugh")?;
        let action = action.unwrap();
        let reply = action.perform(session.clone(), &surroundings)?;
        let (_, _person, _area) = surroundings.unpack();

        assert!(matches!(reply, Effect::Ok));

        build.close()?;

        Ok(())
    }

    #[test]
    fn it_raises_lol_events() -> Result<()> {
        let mut build = BuildSurroundings::new()?;
        let (session, surroundings) = build.plain().build()?;

        let action = try_parsing(LaughActionParser {}, "lol")?;
        let action = action.unwrap();
        let reply = action.perform(session.clone(), &surroundings)?;
        let (_, _person, _area) = surroundings.unpack();

        assert!(matches!(reply, Effect::Ok));

        build.close()?;

        Ok(())
    }
}
