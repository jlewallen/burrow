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
}

impl ParsesActions for MemoryPlugin {
    fn try_parse_action(&self, i: &str) -> EvaluationResult {
        try_parsing(parser::RecallActionParser {}, i)
    }
}

pub mod model {
    use crate::library::model::*;

    #[derive(Debug, Serialize, ToTaggedJson)]
    #[serde(rename_all = "camelCase")]
    pub struct RecalledMemory {
        pub time: DateTime<Utc>,
        pub key: EntityKey,
        pub gid: EntityGid,
        pub name: String,
    }

    impl From<SpecificMemory> for RecalledMemory {
        fn from(value: SpecificMemory) -> Self {
            let entity = match value.event {
                MemoryEvent::Created(e) => e,
                MemoryEvent::Destroyed(e) => e,
                MemoryEvent::Constructed(e) => e,
            };

            Self {
                time: value.time,
                key: entity.key,
                gid: entity.gid,
                name: entity.name,
            }
        }
    }

    #[derive(Debug, Serialize, ToTaggedJson)]
    #[serde(rename_all = "camelCase")]
    pub struct RecallReply {
        pub memories: Vec<RecalledMemory>,
    }

    impl Reply for RecallReply {}

    impl TryInto<Effect> for RecallReply {
        type Error = TaggedJsonError;

        fn try_into(self) -> std::result::Result<Effect, Self::Error> {
            Ok(Effect::Reply(EffectReply::TaggedJson(
                TaggedJson::new_from(serde_json::to_value(self)?)?,
            )))
        }
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct EntityEvent {
        pub(crate) key: EntityKey,
        pub(crate) gid: EntityGid,
        pub(crate) name: String,
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub enum MemoryEvent {
        Created(EntityEvent),
        Destroyed(EntityEvent),
        Constructed(EntityEvent),
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct SpecificMemory {
        pub time: DateTime<Utc>,
        pub event: MemoryEvent,
    }

    #[derive(Debug, Serialize, Deserialize, Default)]
    pub struct Memory {
        memory: Vec<SpecificMemory>,
    }

    impl Memory {}

    impl Scope for Memory {
        fn scope_key() -> &'static str {
            "memory"
        }
    }

    impl From<Memory> for Vec<SpecificMemory> {
        fn from(value: Memory) -> Self {
            value.memory
        }
    }

    pub fn memories_of(entity: &EntityPtr) -> Result<Vec<SpecificMemory>, DomainError> {
        let memory = entity.scope::<Memory>()?.unwrap_or_default();
        Ok(memory.memory.clone())
    }

    pub fn remember(
        entity: &EntityPtr,
        time: DateTime<Utc>,
        event: MemoryEvent,
    ) -> Result<(), DomainError> {
        let mut memory = entity.scope_mut::<Memory>()?;
        memory.memory.push(SpecificMemory { time, event });
        memory.save()
    }
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
            Ok(RecallReply {
                memories: memories.into_iter().map(|m| m.into()).collect(),
            }
            .try_into()?)
        }
    }
}

pub mod parser {
    use super::actions::*;
    use crate::library::parser::*;

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
    use super::model::*;
    use super::parser::*;
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
            MemoryEvent::Created(EntityEvent {
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
