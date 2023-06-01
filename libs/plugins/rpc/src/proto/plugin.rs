use super::fsm::{Machine, Transition};
use super::*;
use super::{Payload, PayloadMessage, Query};
use anyhow::Result;
use tracing::warn;

const DEFAULT_DEPTH: u32 = 2;

type PluginTransition = Transition<PluginState, Query>;

#[derive(Debug, PartialEq, Eq)]
pub enum PluginState {
    Uninitialized,
    Initialized,
    Failed,
    Resolving,
}

type PluginMachine = Machine<PluginState>;

impl PluginMachine {
    fn new() -> Self {
        Self {
            state: PluginState::Uninitialized,
        }
    }
}

pub trait PluginResponses {
    fn surroundings(surroundings: &Surroundings) -> PluginTransition;
}

pub struct DefaultResponses {}

impl PluginResponses for DefaultResponses {
    fn surroundings(surroundings: &Surroundings) -> PluginTransition {
        let keys = match &surroundings {
            Surroundings::Living {
                world,
                living,
                area,
            } => vec![world, living, area],
        };

        let lookups = keys.into_iter().map(|k| LookupBy::Key(k.clone())).collect();
        let lookup = Query::Lookup(DEFAULT_DEPTH, lookups);
        PluginTransition::Send(lookup, PluginState::Resolving)
    }
}

#[derive(Debug)]
pub struct PluginProtocol<R>
where
    R: PluginResponses,
{
    session_key: Option<String>,
    machine: PluginMachine,
    _marker: std::marker::PhantomData<R>,
}

impl<R> PluginProtocol<R>
where
    R: PluginResponses,
{
    pub fn new() -> Self {
        Self {
            session_key: None,
            machine: PluginMachine::new(),
            _marker: Default::default(),
        }
    }

    #[cfg(test)]
    pub fn new_with_session_key(session_key: String) -> Self {
        Self {
            session_key: Some(session_key),
            machine: PluginMachine::new(),
            _marker: Default::default(),
        }
    }

    #[cfg(test)]
    pub fn session_key(&self) -> Option<&str> {
        self.session_key.as_deref()
    }

    pub fn message<B>(&self, body: B) -> Message<B> {
        Message {
            session_key: self.session_key.clone().expect("A session key is required"),
            body,
        }
    }

    pub fn apply(
        &mut self,
        message: &PayloadMessage,
        sender: &mut Sender<QueryMessage>,
    ) -> Result<()> {
        let transition = self.handle(message).map_message(|m| QueryMessage {
            session_key: self.session_key.as_ref().unwrap().clone(),
            body: Some(m),
        });

        self.machine.apply(transition, sender)?;

        Ok(())
    }

    fn handle(&mut self, message: &PayloadMessage) -> PluginTransition {
        match (&self.machine.state, &message.body) {
            (PluginState::Uninitialized, Payload::Initialize(session_key)) => {
                self.session_key = Some(session_key.to_owned());

                PluginTransition::Direct(PluginState::Initialized)
            }
            (PluginState::Initialized, Payload::Surroundings(surroundings)) => {
                R::surroundings(surroundings)
            }
            (PluginState::Resolving, Payload::Resolved(_entities)) => PluginTransition::None,
            (PluginState::Failed, payload) => {
                warn!("(failed) {:?}", &payload);

                PluginTransition::None
            }
            (state, message) => {
                warn!("(failing) {:?} {:?}", state, message);

                PluginTransition::Direct(PluginState::Failed)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use tracing::*;

    use crate::proto::{plugin::PluginState, Payload};

    use super::*;

    type TestPlugin = PluginProtocol<DefaultResponses>;

    #[tokio::test]
    async fn test_initialize() -> anyhow::Result<()> {
        let mut proto = TestPlugin::new_with_session_key("session-key".to_owned());

        assert_eq!(proto.machine.state, PluginState::Uninitialized);

        let session_key = proto.session_key().unwrap().to_owned();
        let initialize = Payload::Initialize(session_key);
        let mut sender = Default::default();
        proto.apply(&proto.message(initialize), &mut sender)?;

        assert_eq!(proto.machine.state, PluginState::Initialized);
        assert!(sender.queue.is_empty());

        Ok(())
    }
}
