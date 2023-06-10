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
    machine: ServerMachine,
}

impl ServerProtocol {
    pub fn new() -> Self {
        Self {
            machine: ServerMachine::new(),
        }
    }

    pub fn completed(&self) -> Completed {
        self.machine.state.completed()
    }

    pub fn apply(
        &mut self,
        message: &Query,
        sender: &mut Sender<Payload>,
        services: &dyn Services,
    ) -> Result<()> {
        let _span = span!(Level::INFO, "server").entered();
        let transition = self.handle(message, services)?;

        self.machine.apply(transition, sender)?;

        Ok(())
    }

    fn handle(&mut self, message: &Query, services: &dyn Services) -> Result<ServerTransition> {
        match (&self.machine.state, &message) {
            (ServerState::Initializing, _) => Ok(ServerTransition::Send(
                Payload::Initialize,
                ServerState::Initialized,
            )),
            (ServerState::Initialized, Query::Bootstrap) => Ok(ServerTransition::None),
            (ServerState::Initialized, Query::Lookup(depth, lookup)) => {
                let resolved = services.lookup(*depth, lookup)?;

                Ok(ServerTransition::Send(
                    Payload::Resolved(resolved),
                    ServerState::Waiting,
                ))
            }
            (ServerState::Waiting, Query::Complete) => {
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

    use crate::{server::ServerState, EntityJson, LookupBy, Payload};

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
        let mut proto = ServerProtocol::new();

        assert_eq!(proto.machine.state, ServerState::Initializing);

        let mut sender = Default::default();
        proto.apply(&crate::Query::Bootstrap, &mut sender, &DummyServer {})?;

        assert_eq!(proto.machine.state, ServerState::Initialized);

        assert_eq!(sender.iter().next(), Some(&Payload::Initialize));

        Ok(())
    }
}
