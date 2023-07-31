use kernel::{DomainError, Entry};

use crate::memory::model::Memory;

use self::model::{MemoryEvent, SpecificMemory};

pub mod model {
    use anyhow::Result;
    use chrono::{DateTime, Utc};
    use kernel::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize)]
    pub struct ItemEvent {
        key: EntityKey,
        name: String,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub enum MemoryEvent {
        Created(ItemEvent),
        Destroyed(ItemEvent),
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct SpecificMemory {
        time: DateTime<Utc>,
        event: MemoryEvent,
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
}

use model::*;

fn memories_of(entity: &Entry) -> Result<Vec<SpecificMemory>, DomainError> {
    let memory = entity.scope::<Memory>()?;
    todo!()
}

fn remember(entity: &Entry, event: MemoryEvent) -> Result<(), DomainError> {
    // let mut usernames = world.scope_mut::<Usernames>()?;
    // usernames.set(username, key);
    // usernames.save()
    todo!()
}
