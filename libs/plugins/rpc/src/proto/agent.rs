use super::fsm::{Machine, Transition};
use super::*;
use super::{Payload, PayloadMessage, Query};
use anyhow::Result;
use tracing::warn;

const DEFAULT_DEPTH: u32 = 2;

type AgentTransition = Transition<AgentState, Query>;

#[derive(Debug, PartialEq, Eq)]
pub enum AgentState {
    Uninitialized,
    Initialized,
    Failed,
    Resolving,
}

type AgentMachine = Machine<AgentState>;

impl AgentMachine {
    fn new() -> Self {
        Self {
            state: AgentState::Uninitialized,
        }
    }
}

pub trait AgentResponses {
    fn surroundings(surroundings: &Surroundings) -> AgentTransition;
}

#[derive(Debug)]
pub struct DefaultResponses {}

impl AgentResponses for DefaultResponses {
    fn surroundings(surroundings: &Surroundings) -> AgentTransition {
        let keys = match &surroundings {
            Surroundings::Living {
                world,
                living,
                area,
            } => vec![world, living, area],
        };

        let lookups = keys.into_iter().map(|k| LookupBy::Key(k.clone())).collect();
        let lookup = Query::Lookup(DEFAULT_DEPTH, lookups);
        AgentTransition::Send(lookup, AgentState::Resolving)
    }
}

#[derive(Debug)]
pub struct AgentProtocol<R>
where
    R: AgentResponses,
{
    session_key: Option<SessionKey>,
    machine: AgentMachine,
    _marker: std::marker::PhantomData<R>,
}

impl<R> AgentProtocol<R>
where
    R: AgentResponses,
{
    pub fn new() -> Self {
        Self {
            session_key: None,
            machine: AgentMachine::new(),
            _marker: Default::default(),
        }
    }

    #[cfg(test)]
    pub fn new_with_session_key(session_key: SessionKey) -> Self {
        Self {
            session_key: Some(session_key),
            machine: AgentMachine::new(),
            _marker: Default::default(),
        }
    }

    #[cfg(test)]
    pub fn session_key(&self) -> Option<&SessionKey> {
        self.session_key.as_ref()
    }

    pub fn apply(
        &mut self,
        message: &PayloadMessage,
        sender: &mut Sender<QueryMessage>,
    ) -> Result<()> {
        let _span = span!(Level::INFO, "agent").entered();
        let transition = self.handle(message).map_message(|m| QueryMessage {
            session_key: self.session_key.as_ref().unwrap().clone(),
            body: Some(m),
        });

        self.machine.apply(transition, sender)?;

        Ok(())
    }

    fn handle(&mut self, message: &PayloadMessage) -> AgentTransition {
        match (&self.machine.state, &message.body) {
            (AgentState::Uninitialized, Payload::Initialize(session_key)) => {
                self.session_key = Some(session_key.to_owned());

                AgentTransition::Direct(AgentState::Initialized)
            }
            (AgentState::Initialized, Payload::Surroundings(surroundings)) => {
                R::surroundings(surroundings)
            }
            (AgentState::Resolving, Payload::Resolved(_entities)) => {
                AgentTransition::Send(Query::Complete, AgentState::Initialized)
            }
            (AgentState::Failed, payload) => {
                warn!("(failed) {:?}", &payload);

                AgentTransition::None
            }
            (state, message) => {
                warn!("(failing) {:?} {:?}", state, message);

                AgentTransition::Direct(AgentState::Failed)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use tracing::*;

    use crate::proto::{agent::AgentState, Payload};

    use super::*;

    type TestAgent = AgentProtocol<DefaultResponses>;

    #[tokio::test]
    async fn test_initialize() -> anyhow::Result<()> {
        let session_key = SessionKey::new("session-key");
        let mut proto = TestAgent::new_with_session_key(session_key.clone());

        assert_eq!(proto.machine.state, AgentState::Uninitialized);

        let session_key = proto.session_key().unwrap().to_owned();
        let initialize = Payload::Initialize(session_key.clone());
        let mut sender = Default::default();
        proto.apply(&session_key.message(initialize), &mut sender)?;

        assert_eq!(proto.machine.state, AgentState::Initialized);
        assert!(sender.queue.is_empty());

        Ok(())
    }
}
