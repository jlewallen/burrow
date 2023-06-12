use anyhow::Result;
use tracing::warn;

use super::{fsm::*, *};

pub const DEFAULT_DEPTH: u32 = 2;

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

pub trait Agent {
    fn ready(&mut self) -> Result<()>;
}

#[derive(Debug)]
pub struct AgentProtocol<R>
where
    R: AgentResponses,
{
    machine: AgentMachine,
    _marker: std::marker::PhantomData<R>,
}

impl<R> AgentProtocol<R>
where
    R: AgentResponses,
{
    pub fn new() -> Self {
        Self {
            machine: AgentMachine::new(),
            _marker: Default::default(),
        }
    }

    pub fn apply<AgentT>(
        &mut self,
        message: &Payload,
        sender: &mut Sender<Query>,
        agent: &mut AgentT,
    ) -> Result<()>
    where
        AgentT: Agent,
    {
        let _span = span!(Level::INFO, "agent").entered();
        let transition = self.handle(message, agent)?;

        self.machine.apply(transition, sender)
    }

    fn handle<AgentT>(&mut self, message: &Payload, agent: &mut AgentT) -> Result<AgentTransition>
    where
        AgentT: Agent,
    {
        match (&self.machine.state, &message) {
            (AgentState::Uninitialized, Payload::Initialize) => {
                Ok(AgentTransition::Direct(AgentState::Initialized))
            }
            (AgentState::Initialized, Payload::Initialize) => Ok(AgentTransition::None),
            (AgentState::Initialized, Payload::Surroundings(surroundings)) => {
                Ok(R::surroundings(surroundings))
            }
            (AgentState::Resolving, Payload::Resolved(_entities)) => {
                agent.ready()?;

                Ok(AgentTransition::Send(
                    Query::Complete,
                    AgentState::Initialized,
                ))
            }
            (AgentState::Failed, payload) => {
                warn!("(failed) {:?}", &payload);

                Ok(AgentTransition::None)
            }
            (state, message) => {
                warn!("(failing) {:?} {:?}", state, message);

                Ok(AgentTransition::Direct(AgentState::Failed))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use tracing::*;

    use super::*;

    type TestAgentProtocol = AgentProtocol<DefaultResponses>;

    struct TestAgent {}

    impl Agent for TestAgent {
        fn ready(&mut self) -> Result<()> {
            Ok(())
        }
    }

    #[test]
    fn test_initialize() -> anyhow::Result<()> {
        let mut proto = TestAgentProtocol::new();

        assert_eq!(proto.machine.state, AgentState::Uninitialized);

        let initialize = Payload::Initialize;
        let mut sender = Default::default();
        proto.apply(&initialize, &mut sender, &mut TestAgent {})?;

        assert_eq!(proto.machine.state, AgentState::Initialized);
        assert!(sender.queue.is_empty());

        Ok(())
    }
}
