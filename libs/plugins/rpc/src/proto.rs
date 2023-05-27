use serde::{Deserialize, Serialize};

pub type SessionKey = String;

pub type EntityKey = String;

pub type EntityJson = serde_json::Value;

#[derive(Debug, Serialize, Deserialize)]
pub struct EntityUpdate {
    entity_key: EntityKey,
    entity: EntityJson,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Event {
    Arrived,
    Left,
    Held,
    Dropped,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Reply {
    Done,
    NotFound,
    Impossible,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Find {}

#[derive(Debug, Serialize, Deserialize)]
pub enum Try {
    CanMove,
    Moved,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Permission {}

#[derive(Debug, Serialize, Deserialize)]
pub enum Hook {}

#[derive(Debug, Serialize, Deserialize)]
pub enum LookupBy {
    Key(EntityKey),
    Gid(u64),
}

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
pub struct QueryMessage {
    session_key: SessionKey,
    query: Option<Query>,
}

impl QueryMessage {
    #[cfg(test)]
    fn into_tuple(self) -> (SessionKey, Option<Query>) {
        (self.session_key, self.query)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Surroundings {
    Living {
        world: EntityJson,
        living: EntityJson,
        area: EntityJson,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Payload {
    Initialize, /* Complete */

    Evaluate(String, Surroundings), /* Reply */

    Entity(Option<EntityJson>),
    Found(Vec<EntityJson>),

    Permission(Permission),

    Hook(Hook),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PayloadMessage {
    session_key: SessionKey,
    payload: Payload,
}

impl PayloadMessage {
    #[cfg(test)]
    fn into_tuple(self) -> (SessionKey, Payload) {
        (self.session_key, self.payload)
    }
}

#[allow(dead_code)]
#[allow(unused_variables)]
#[cfg(test)]
mod tests {
    use anyhow::Result;

    use super::{Payload, PayloadMessage, Query, QueryMessage};

    mod plugin {
        use super::*;
        use tracing::*;

        #[derive(Debug)]
        enum PluginTransition {
            Direct(PluginState),
            Send(Query, PluginState),
        }

        #[derive(Debug, PartialEq, Eq)]
        enum PluginState {
            Uninitialized,
            Initialized,
            Failed,
        }

        #[derive(Debug)]
        pub struct PluginProtocol {
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
            pub fn apply(&mut self, message: PayloadMessage) -> Result<()> {
                match self.handle(message) {
                    PluginTransition::Direct(state) => {
                        info!("{:?} -> {:?}", &self.state, &state);
                        self.state = state;
                        Ok(())
                    }
                    PluginTransition::Send(message, state) => {
                        info!("{:?} Send", &message);
                        info!("{:?} -> {:?}", &self.state, &state);
                        self.state = state;
                        Ok(())
                    }
                }
            }

            fn handle(&self, message: PayloadMessage) -> PluginTransition {
                let (session_key, payload) = message.into_tuple();
                match (&self.state, payload) {
                    (PluginState::Uninitialized, Payload::Initialize) => {
                        PluginTransition::Direct(PluginState::Initialized)
                    }
                    (PluginState::Initialized, _) => todo!(),
                    (PluginState::Failed, _) => todo!(),
                    (_, _) => PluginTransition::Direct(PluginState::Failed),
                }
            }
        }

        /*
        struct StateMachine<S> {
            state: S,
        }

        struct Uninitialized {}

        struct Failed {}

        impl StateMachine<Uninitialized> {
            fn new() -> Self {
                Self {
                    state: Uninitialized {},
                }
            }
        }

        impl From<StateMachine<Uninitialized>> for StateMachine<Failed> {
            fn from(val: StateMachine<Uninitialized>) -> StateMachine<Failed> {
                StateMachine { state: Failed {} }
            }
        }
        */

        #[cfg(test)]
        mod tests {
            use crate::proto::{tests::plugin::PluginState, Payload, PayloadMessage, SessionKey};

            use super::PluginProtocol;

            #[test]
            fn test_initialize() -> anyhow::Result<()> {
                let mut proto = PluginProtocol::default();
                let session_key: SessionKey = "session-key".to_owned();

                assert_eq!(proto.state, PluginState::Uninitialized);

                proto.apply(PayloadMessage {
                    session_key: session_key.clone(),
                    payload: Payload::Initialize,
                })?;

                Ok(())
            }
        }
    }

    mod server {
        use super::*;
        use tracing::*;

        #[derive(Debug)]
        enum ServerTransition {
            Direct(ServerState),
            Send(Payload, ServerState),
        }

        #[derive(Debug, PartialEq, Eq)]
        enum ServerState {
            Initializing,
            Initialized,
            Failed,
        }

        #[derive(Debug)]
        pub struct ServerProtocol {
            state: ServerState,
        }

        impl Default for ServerProtocol {
            fn default() -> Self {
                Self {
                    state: ServerState::Initializing,
                }
            }
        }

        impl ServerProtocol {
            pub fn apply(&mut self, message: QueryMessage) -> Result<()> {
                match self.handle(message) {
                    ServerTransition::Direct(state) => {
                        info!("{:?} -> {:?}", &self.state, &state);
                        self.state = state;
                        Ok(())
                    }
                    ServerTransition::Send(sending, state) => {
                        info!("{:?} Send", &sending);
                        info!("{:?} -> {:?}", &self.state, &state);
                        self.state = state;
                        Ok(())
                    }
                }
            }

            fn handle(&self, message: QueryMessage) -> ServerTransition {
                let (session_key, query) = message.into_tuple();
                match (&self.state, query) {
                    (ServerState::Initializing, _) => {
                        ServerTransition::Send(Payload::Initialize, ServerState::Initialized)
                    }
                    (ServerState::Failed, _) => todo!(),
                    (ServerState::Initialized, _) => todo!(),
                }
            }
        }

        /*
        struct StateMachine<S> {
            state: S,
        }

        struct Initializing {}

        struct Failed {}

        impl StateMachine<Initializing> {
            fn new() -> Self {
                Self {
                    state: Initializing {},
                }
            }
        }

        impl From<StateMachine<Initializing>> for StateMachine<Failed> {
            fn from(val: StateMachine<Initializing>) -> StateMachine<Failed> {
                StateMachine { state: Failed {} }
            }
        }
        */

        #[cfg(test)]
        mod tests {
            use crate::proto::{tests::server::ServerState, QueryMessage, SessionKey};

            use super::ServerProtocol;

            #[test]
            fn test_initialize() -> anyhow::Result<()> {
                let mut server = ServerProtocol::default();
                let session_key: SessionKey = "session-key".to_owned();

                assert_eq!(server.state, ServerState::Initializing);

                server.apply(QueryMessage {
                    session_key: session_key.clone(),
                    query: None,
                })?;

                Ok(())
            }
        }
    }

    use plugin::PluginProtocol;
    use server::ServerProtocol;

    fn create() -> Result<(ServerProtocol, PluginProtocol)> {
        Ok((ServerProtocol::default(), PluginProtocol::default()))
    }
}
