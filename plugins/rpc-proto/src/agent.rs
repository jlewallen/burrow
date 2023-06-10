use anyhow::Result;
use tracing::warn;

use super::{fsm::*, *};

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

    pub fn apply(&mut self, message: &Payload, sender: &mut Sender<Query>) -> Result<()> {
        let _span = span!(Level::INFO, "agent").entered();
        let transition = self.handle(message);

        self.machine.apply(transition, sender)?;

        Ok(())
    }

    fn handle(&mut self, message: &Payload) -> AgentTransition {
        match (&self.machine.state, &message) {
            (AgentState::Uninitialized, Payload::Initialize) => {
                AgentTransition::Direct(AgentState::Initialized)
            }
            (AgentState::Initialized, Payload::Initialize) => AgentTransition::None,
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

    use super::*;

    type TestAgent = AgentProtocol<DefaultResponses>;

    #[test]
    fn test_initialize() -> anyhow::Result<()> {
        let mut proto = TestAgent::new();

        assert_eq!(proto.machine.state, AgentState::Uninitialized);

        let initialize = Payload::Initialize;
        let mut sender = Default::default();
        proto.apply(&initialize, &mut sender)?;

        assert_eq!(proto.machine.state, AgentState::Initialized);
        assert!(sender.queue.is_empty());

        Ok(())
    }
}
