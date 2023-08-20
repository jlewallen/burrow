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

    fn sources(&self) -> Vec<Box<dyn ActionSource>> {
        vec![Box::new(ActionSources::default())]
    }
}

impl ParsesActions for EmotePlugin {
    fn try_parse_action(&self, i: &str) -> EvaluationResult {
        try_parsing(parser::LaughActionParser {}, i)
    }
}

#[derive(Default)]
pub struct ActionSources {}

impl ActionSource for ActionSources {
    fn try_deserialize_action(
        &self,
        tagged: &TaggedJson,
    ) -> Result<Option<Box<dyn Action>>, serde_json::Error> {
        if let Some(a) = actions::LaughAction::from_tagged_json(tagged)? {
            return Ok(Some(Box::new(a)));
        }
        Ok(None)
    }
}

pub mod model {
    pub use kernel::common::Emoting;
}

pub mod actions {
    use crate::{library::actions::*, looking::model::Observe};

    #[action]
    pub struct LaughAction {}

    impl Action for LaughAction {
        fn is_read_only() -> bool {
            true
        }

        fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
            let (_, living, area) = surroundings.unpack();

            session.raise(
                Some(living.clone()),
                Audience::Area(area.key().clone()),
                Raising::TaggedJson(
                    Emoting::Laugh(Emoted::new(
                        (&living).observe(&living)?.expect("No observed entity"),
                    ))
                    .to_tagged_json()?,
                ),
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
        let (_surroundings, effect) = parse_and_perform(LaughActionParser {}, "laugh")?;

        assert!(matches!(effect, Effect::Ok));

        Ok(())
    }

    #[test]
    fn it_raises_lol_events() -> Result<()> {
        let (_surroundings, effect) = parse_and_perform(LaughActionParser {}, "lol")?;

        assert!(matches!(effect, Effect::Ok));

        Ok(())
    }
}
