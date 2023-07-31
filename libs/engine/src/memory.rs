pub mod model {
    use anyhow::Result;
    use chrono::{DateTime, Utc};
    use serde::{Deserialize, Serialize};

    use kernel::*;

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct ItemEvent {
        pub key: EntityKey,
        pub gid: EntityGid,
        pub name: String,
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub enum MemoryEvent {
        Created(ItemEvent),
        Destroyed(ItemEvent),
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

    impl Needs<SessionRef> for Memory {
        fn supply(&mut self, _session: &SessionRef) -> Result<()> {
            Ok(())
        }
    }

    impl Scope for Memory {
        fn serialize(&self) -> Result<serde_json::Value> {
            Ok(serde_json::to_value(self)?)
        }

        fn scope_key() -> &'static str {
            "memory"
        }
    }

    impl Into<Vec<SpecificMemory>> for Memory {
        fn into(self) -> Vec<SpecificMemory> {
            self.memory
        }
    }

    pub fn memories_of(entity: &Entry) -> Result<Vec<SpecificMemory>, DomainError> {
        let memory = entity.scope::<Memory>()?;
        Ok(memory.memory.clone())
    }

    pub fn remember(
        entity: &Entry,
        time: DateTime<Utc>,
        event: MemoryEvent,
    ) -> Result<(), DomainError> {
        let mut memory = entity.scope_mut::<Memory>()?;
        memory.memory.push(SpecificMemory { time, event });
        memory.save()
    }
}
