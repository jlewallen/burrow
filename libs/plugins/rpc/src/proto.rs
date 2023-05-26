use serde::{Deserialize, Serialize};

pub type SessionKey = String;

pub type EntityKey = String;

pub type EntityJson = serde_json::Value;

#[derive(Serialize, Deserialize)]
pub struct EntityUpdate {
    entity_key: EntityKey,
    entity: EntityJson,
}

#[derive(Serialize, Deserialize)]
pub enum Event {
    Arrived,
    Left,
    Held,
    Dropped,
}

#[derive(Serialize, Deserialize)]
pub enum Reply {
    Done,
    NotFound,
    Impossible,
}

#[derive(Serialize, Deserialize)]
pub enum Find {}

#[derive(Serialize, Deserialize)]
pub enum Try {
    CanMove,
    Moved,
}

#[derive(Serialize, Deserialize)]
pub enum Permission {}

#[derive(Serialize, Deserialize)]
pub enum Hook {}

#[derive(Serialize, Deserialize)]
pub enum LookupBy {
    Key(EntityKey),
    Gid(u64),
}

#[derive(Serialize, Deserialize)]
pub enum Query {
    Complete,

    Update(EntityUpdate),
    Raise(Event),
    Chain(String),
    Reply(Reply),

    Permission(Try),

    Lookup(LookupBy),
    Find(Find),

    Try(Try),
}

#[derive(Serialize, Deserialize)]
pub struct QueryMessage {
    session_key: SessionKey,
    query: Option<Query>,
}

impl QueryMessage {
    fn into_tuple(self) -> (SessionKey, Option<Query>) {
        (self.session_key, self.query)
    }
}

#[derive(Serialize, Deserialize)]
pub enum Surroundings {
    Living {
        world: EntityJson,
        living: EntityJson,
        area: EntityJson,
    },
}

#[derive(Serialize, Deserialize)]
pub enum Payload {
    Initialize, /* Complete */

    Evaluate(String, Surroundings), /* Reply */

    Entity(Option<EntityJson>),
    Found(Vec<EntityJson>),

    Permission(Permission),

    Hook(Hook),
}

#[derive(Serialize, Deserialize)]
pub struct PayloadMessage {
    session_key: SessionKey,
    payload: Payload,
}

impl PayloadMessage {
    fn into_tuple(self) -> (SessionKey, Payload) {
        (self.session_key, self.payload)
    }
}

#[allow(dead_code)]
#[allow(unused_variables)]
#[cfg(test)]
mod tests {
    use anyhow::Result;

    use crate::proto::SessionKey;

    use super::{Payload, PayloadMessage, Query, QueryMessage};

    enum PluginTransition {
        Direct(PluginState),
    }

    enum PluginState {
        Uninitialized,
    }

    impl PluginState {
        fn handle(&self, payload: Payload) -> PluginTransition {
            match self {
                PluginState::Uninitialized => match payload {
                    Payload::Initialize => todo!(),
                    Payload::Evaluate(_, _) => todo!(),
                    Payload::Entity(_) => todo!(),
                    Payload::Found(_) => todo!(),
                    Payload::Permission(_) => todo!(),
                    Payload::Hook(_) => todo!(),
                },
            }
        }
    }

    struct PluginProtocol {
        state: PluginState,
    }

    impl Default for PluginProtocol {
        fn default() -> Self {
            Self {
                state: PluginState::Uninitialized,
            }
        }
    }

    impl PluginProtocol {
        fn handle(&self, payload: PayloadMessage) -> Self {
            let (session_key, payload) = payload.into_tuple();
            Self {
                state: match self.state.handle(payload) {
                    PluginTransition::Direct(state) => state,
                },
            }
        }
    }

    enum ServerTransition {
        Direct(ServerState),
    }

    enum ServerState {
        SendingInitialize,
    }

    impl ServerState {
        fn handle(&self, query: Option<Query>) -> ServerTransition {
            match self {
                ServerState::SendingInitialize => match query {
                    Some(Query::Complete) => todo!(),
                    Some(Query::Update(_)) => todo!(),
                    Some(Query::Raise(_)) => todo!(),
                    Some(Query::Chain(_)) => todo!(),
                    Some(Query::Reply(_)) => todo!(),
                    Some(Query::Permission(_)) => todo!(),
                    Some(Query::Lookup(_)) => todo!(),
                    Some(Query::Find(_)) => todo!(),
                    Some(Query::Try(_)) => todo!(),
                    None => ServerTransition::Direct(ServerState::SendingInitialize),
                },
            }
        }
    }

    struct ServerProtocol {
        state: ServerState,
    }

    impl Default for ServerProtocol {
        fn default() -> Self {
            Self {
                state: ServerState::SendingInitialize,
            }
        }
    }

    impl ServerProtocol {
        fn handle(&self, query: QueryMessage) -> Self {
            let (session_key, query) = query.into_tuple();
            Self {
                state: match self.state.handle(query) {
                    ServerTransition::Direct(state) => state,
                },
            }
        }
    }

    fn create() -> Result<(ServerProtocol, PluginProtocol)> {
        Ok((ServerProtocol::default(), PluginProtocol::default()))
    }

    #[test]
    fn test_initialize() -> Result<()> {
        let (server, plugin) = create()?;

        let session_key: SessionKey = "session-key".to_owned();

        server.handle(QueryMessage {
            session_key: session_key.clone(),
            query: None,
        });

        Ok(())
    }
}
