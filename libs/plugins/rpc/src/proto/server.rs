use super::{fsm::*, *};
use anyhow::{anyhow, Result};

type ServerTransition = Transition<ServerState, Payload>;

#[derive(Debug, PartialEq, Eq)]
enum ServerState {
    Initializing,
    Initialized,
    Failed,
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
    session_key: String,
    machine: ServerMachine,
}

impl ServerProtocol {
    pub fn new() -> Self {
        Self {
            session_key: "session-key".to_owned(),
            machine: ServerMachine::new(),
        }
    }

    pub fn message<B>(&self, body: B) -> Message<B> {
        Message {
            session_key: self.session_key.clone(),
            body,
        }
    }

    pub fn apply(
        &mut self,
        message: &QueryMessage,
        sender: &mut Sender<PayloadMessage>,
        server: &dyn Server,
    ) -> Result<()> {
        let transition = self
            .handle(message, server)?
            .map_message(|m| PayloadMessage {
                session_key: self.session_key.clone(),
                body: m,
            });

        self.machine.apply(transition, sender)?;

        Ok(())
    }

    fn handle(&mut self, message: &QueryMessage, server: &dyn Server) -> Result<ServerTransition> {
        match (&self.machine.state, &message.body) {
            (ServerState::Initializing, _) => Ok(ServerTransition::Send(
                Payload::Initialize(message.session_key.clone()),
                ServerState::Initialized,
            )),
            (ServerState::Initialized, Some(Query::Lookup(depth, lookup))) => {
                let resolved = server.lookup(*depth, lookup)?;

                Ok(ServerTransition::SendOnly(Payload::Resolved(resolved)))
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

pub trait Server {
    fn lookup(
        &self,
        depth: u32,
        lookup: &Vec<LookupBy>,
    ) -> Result<Vec<(LookupBy, Option<EntityJson>)>>;
}

pub struct AlwaysErrorsServer {}

impl Server for AlwaysErrorsServer {
    fn lookup(
        &self,
        _depth: u32,
        _lookup: &Vec<LookupBy>,
    ) -> Result<Vec<(LookupBy, Option<EntityJson>)>> {
        Err(anyhow!("This server always errors"))
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    #[allow(unused_imports)]
    use tracing::*;

    use crate::proto::{server::ServerState, EntityJson, LookupBy, Payload};

    use super::{Server, ServerProtocol};

    struct DummyServer {}

    impl Server for DummyServer {
        fn lookup(
            &self,
            _depth: u32,
            _lookup: &Vec<LookupBy>,
        ) -> Result<Vec<(LookupBy, Option<EntityJson>)>> {
            Ok(vec![])
        }
    }

    #[tokio::test]
    async fn test_initialize() -> anyhow::Result<()> {
        let mut proto = ServerProtocol::new();

        assert_eq!(proto.machine.state, ServerState::Initializing);

        let mut sender = Default::default();
        let start = proto.message(None);
        proto.apply(&start, &mut sender, &DummyServer {})?;

        assert_eq!(proto.machine.state, ServerState::Initialized);

        assert_eq!(
            sender.bodies().next(),
            Some(&Payload::Initialize("session-key".to_owned()))
        );

        Ok(())
    }
}
