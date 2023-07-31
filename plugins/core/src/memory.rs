use crate::library::plugin::*;

#[derive(Default)]
pub struct MemoryPluginFactory {}

impl PluginFactory for MemoryPluginFactory {
    fn create_plugin(&self) -> Result<Box<dyn Plugin>> {
        Ok(Box::new(MemoryPlugin {}))
    }

    fn stop(&self) -> Result<()> {
        Ok(())
    }
}

#[derive(Default)]
pub struct MemoryPlugin {}

impl Plugin for MemoryPlugin {
    fn plugin_key() -> &'static str
    where
        Self: Sized,
    {
        "memory"
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

impl ParsesActions for MemoryPlugin {
    fn try_parse_action(&self, i: &str) -> EvaluationResult {
        try_parsing(parser::RecallActionParser {}, i)
    }
}

pub mod model {
    use crate::library::model::*;
    use engine::SpecificMemory;
    pub use engine::{memories_of, remember, MemoryEvent};

    #[derive(Debug, Serialize, ToJson)]
    #[serde(rename_all = "camelCase")]
    pub struct RecalledMemory {
        pub time: DateTime<Utc>,
        pub key: EntityKey,
        pub gid: EntityGid,
        pub name: String,
    }

    impl From<SpecificMemory> for RecalledMemory {
        fn from(value: SpecificMemory) -> Self {
            let item = match value.event {
                MemoryEvent::Created(e) => e,
                MemoryEvent::Destroyed(e) => e,
            };

            Self {
                time: value.time,
                key: item.key,
                gid: item.gid,
                name: item.name,
            }
        }
    }

    #[derive(Debug, Serialize, ToJson)]
    #[serde(rename_all = "camelCase")]
    pub struct RecallReply {
        pub memories: Vec<RecalledMemory>,
    }

    impl Reply for RecallReply {}
}

pub mod actions {
    use super::model::*;
    use crate::library::actions::*;

    #[action]
    pub struct RecallAction {}

    impl Action for RecallAction {
        fn is_read_only() -> bool {
            true
        }

        fn perform(&self, _session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
            let (_world, living, _area) = surroundings.unpack();
            let memories = memories_of(&living)?;
            Ok(Effect::Reply(EffectReply::Instance(Rc::new(RecallReply {
                memories: memories.into_iter().map(|m| m.into()).collect(),
            }))))
        }
    }
}

pub mod parser {
    use crate::library::parser::*;

    use super::actions::*;

    pub struct RecallActionParser {}

    impl ParsesActions for RecallActionParser {
        fn try_parse_action(&self, i: &str) -> EvaluationResult {
            let (_, action) = map(tag("recall"), |_| {
                Box::new(RecallAction {}) as Box<dyn Action>
            })(i)?;

            Ok(Some(action))
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;
    use chrono::Utc;
    use engine::ItemEvent;

    use super::model::*;
    use super::parser::*;
    use super::*;
    use crate::library::plugin::try_parsing;
    use crate::library::tests::*;

    #[test]
    fn it_recalls_when_no_memories() -> Result<()> {
        let mut build = BuildSurroundings::new()?;
        let (session, surroundings) = build.plain().build()?;

        let action = try_parsing(RecallActionParser {}, "recall")?;
        let action = action.unwrap();
        let reply = action.perform(session.clone(), &surroundings)?;
        let (_, _person, _area) = surroundings.unpack();

        insta::assert_json_snapshot!(reply.to_debug_json()?);

        build.close()?;

        Ok(())
    }

    #[test]
    fn it_recalls_when_some_memories() -> Result<()> {
        let mut build = BuildSurroundings::new()?;
        let (session, surroundings) = build.build()?;

        let (_, living, _) = surroundings.clone().unpack();
        let time = Utc.with_ymd_and_hms(1982, 4, 23, 0, 0, 0).unwrap();
        remember(
            &living,
            time,
            MemoryEvent::Created(ItemEvent {
                key: session.new_key(),
                gid: EntityGid::new(3),
                name: "Doesn't actually exist".to_owned(),
            }),
        )?;

        let action = try_parsing(RecallActionParser {}, "recall")?;
        let action = action.unwrap();
        let reply = action.perform(session.clone(), &surroundings)?;
        let (_, _person, _area) = surroundings.unpack();

        insta::assert_json_snapshot!(reply.to_debug_json()?);

        build.close()?;

        Ok(())
    }
}
