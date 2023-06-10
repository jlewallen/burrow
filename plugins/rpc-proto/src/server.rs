use anyhow::{anyhow, Result};

use super::{fsm::*, *};

type ServerTransition = Transition<ServerState, Payload>;

pub enum Completed {
    Busy,
    Continue,
}

#[derive(Debug, PartialEq, Eq)]
enum ServerState {
    Initializing,
    Initialized,
    Waiting,
    Failed,
}

impl ServerState {
    pub fn completed(&self) -> Completed {
        match &self {
            ServerState::Initializing => Completed::Busy,
            ServerState::Initialized => Completed::Continue,
            ServerState::Waiting => Completed::Busy,
            ServerState::Failed => Completed::Continue,
        }
    }
}

type ServerMachine = Machine<ServerState>;

impl ServerMachine {
    fn new() -> Self {
        Self {
            state: ServerState::Initializing,
        }
    }
}

#[derive(Debug)]
pub struct ServerProtocol {
    session_key: SessionKey,
    machine: ServerMachine,
}

impl ServerProtocol {
    pub fn new(session_key: SessionKey) -> Self {
        Self {
            session_key,
            machine: ServerMachine::new(),
        }
    }

    pub fn completed(&self) -> Completed {
        self.machine.state.completed()
    }

    pub fn apply(
        &mut self,
        message: &QueryMessage,
        sender: &mut Sender<PayloadMessage>,
        services: &dyn Services,
    ) -> Result<()> {
        let _span = span!(Level::INFO, "server").entered();
        let transition = self
            .handle(message, services)?
            .map_message(|m| PayloadMessage {
                session_key: self.session_key.clone(),
                body: m,
            });

        self.machine.apply(transition, sender)?;

        Ok(())
    }

    fn handle(
        &mut self,
        message: &QueryMessage,
        services: &dyn Services,
    ) -> Result<ServerTransition> {
        match (&self.machine.state, &message.body) {
            (ServerState::Initializing, _) => Ok(ServerTransition::Send(
                Payload::Initialize(message.session_key.clone()),
                ServerState::Initialized,
            )),
            (ServerState::Initialized, None) => Ok(ServerTransition::None),
            (ServerState::Initialized, Some(Query::Lookup(depth, lookup))) => {
                let resolved = services.lookup(*depth, lookup)?;

                Ok(ServerTransition::Send(
                    Payload::Resolved(resolved),
                    ServerState::Waiting,
                ))
            }
            (ServerState::Waiting, Some(Query::Complete)) => {
                Ok(ServerTransition::Direct(ServerState::Initialized))
            }
            (ServerState::Failed, query) => {
                warn!("(failed) {:?}", &query);

                Ok(ServerTransition::None)
            }
            (state, message) => {
                warn!("(failing) {:?} {:?}", state, message);

                Ok(ServerTransition::Direct(ServerState::Failed))
            }
        }
    }
}

pub trait Services {
    fn lookup(
        &self,
        depth: u32,
        lookup: &[LookupBy],
    ) -> Result<Vec<(LookupBy, Option<EntityJson>)>>;
}

pub struct AlwaysErrorsServices {}

impl Services for AlwaysErrorsServices {
    fn lookup(
        &self,
        _depth: u32,
        _lookup: &[LookupBy],
    ) -> Result<Vec<(LookupBy, Option<EntityJson>)>> {
        Err(anyhow!("This server always errors"))
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    #[allow(unused_imports)]
    use tracing::*;

    use crate::{server::ServerState, EntityJson, LookupBy, Payload, SessionKey};

    use super::{ServerProtocol, Services};

    struct DummyServer {}

    impl Services for DummyServer {
        fn lookup(
            &self,
            _depth: u32,
            _lookup: &[LookupBy],
        ) -> Result<Vec<(LookupBy, Option<EntityJson>)>> {
            Ok(vec![])
        }
    }

    #[test]
    fn test_initialize() -> anyhow::Result<()> {
        let session_key = SessionKey::new("session-key");
        let mut proto = ServerProtocol::new(session_key.clone());

        assert_eq!(proto.machine.state, ServerState::Initializing);

        let mut sender = Default::default();
        let start = session_key.message(None);
        proto.apply(&start, &mut sender, &DummyServer {})?;

        assert_eq!(proto.machine.state, ServerState::Initialized);

        assert_eq!(
            sender.bodies().next(),
            Some(&Payload::Initialize(session_key))
        );

        Ok(())
    }
}
