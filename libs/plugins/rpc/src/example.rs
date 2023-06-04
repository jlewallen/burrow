use anyhow::Result;
use tracing::*;

use crate::proto::{
    AgentProtocol, DefaultResponses, Inbox, Payload, PayloadMessage, QueryMessage, Sender,
};

#[derive(Debug)]
pub struct ExampleAgent {
    agent: AgentProtocol<DefaultResponses>,
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

impl Inbox<PayloadMessage, QueryMessage> for ExampleAgent {
    fn deliver(
        &mut self,
        message: &PayloadMessage,
        replies: &mut Sender<QueryMessage>,
    ) -> Result<()> {
        self.agent.apply(message, replies)?;

        self.handle(message.body())?;

        Ok(())
    }
}
