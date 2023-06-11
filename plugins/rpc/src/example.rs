use anyhow::Result;
use tracing::*;

use plugins_rpc_proto::{Agent, AgentProtocol, DefaultResponses, Inbox, Payload, Query, Sender};

#[derive(Debug)]
pub struct ExampleAgent {
    agent: AgentProtocol<DefaultResponses>,
}

impl Default for ExampleAgent {
    fn default() -> Self {
        Self::new()
    }
}

struct EmptyAgent {}

impl Agent for EmptyAgent {
    fn ready(&mut self) -> Result<()> {
        Ok(())
    }
}

impl ExampleAgent {
    pub fn new() -> Self {
        Self {
            agent: AgentProtocol::new(),
        }
    }

    pub fn handle(&mut self, _message: &Payload) -> Result<()> {
        trace!("(handle) {:?}", _message);

        Ok(())
    }
}

impl Inbox<Payload, Query> for ExampleAgent {
    fn deliver(&mut self, message: &Payload, replies: &mut Sender<Query>) -> Result<()> {
        self.agent.apply(message, replies, &mut EmptyAgent {})?;

        self.handle(message)?;

        Ok(())
    }
}
