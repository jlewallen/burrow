use anyhow::Result;
use kernel::Surroundings;
use tracing::*;

use crate::proto::{
    AgentProtocol, AlwaysErrorsServer, DefaultResponses, Inbox, Payload, PayloadMessage,
    QueryMessage, Sender, Server, ServerProtocol, SessionKey,
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

pub struct InProcessServer<P> {
    session_key: SessionKey,
    server: ServerProtocol,
    agent: P,
}

impl InProcessServer<ExampleAgent> {
    pub fn new(session_key: SessionKey) -> Self {
        Self {
            session_key: session_key.clone(),
            server: ServerProtocol::new(session_key),
            agent: ExampleAgent::new(),
        }
    }

    pub fn initialize(&mut self) -> Result<()> {
        self.handle(self.session_key.message(None), &AlwaysErrorsServer {})
    }

    pub fn have_surroundings(
        &mut self,
        surroundings: &Surroundings,
        server: &dyn Server,
    ) -> Result<()> {
        let payload = Payload::Surroundings(surroundings.try_into()?);
        self.send(&self.session_key.message(payload), server)
    }

    fn handle(&mut self, query: QueryMessage, server: &dyn Server) -> Result<()> {
        let mut to_server: Sender<_> = Default::default();
        to_server.send(query)?;
        self.drain(to_server, server)
    }

    fn drain(&mut self, mut to_server: Sender<QueryMessage>, server: &dyn Server) -> Result<()> {
        let mut to_agent: Sender<_> = Default::default();

        while let Some(sending) = to_server.pop() {
            self.server.apply(&sending, &mut to_agent, server)?;
            for message in to_agent.iter() {
                self.deliver(message, &mut to_server)?;
            }
        }

        Ok(())
    }

    fn send(&mut self, message: &PayloadMessage, server: &dyn Server) -> Result<()> {
        let mut to_server: Sender<_> = Default::default();

        self.deliver(message, &mut to_server)?;

        self.drain(to_server, server)
    }

    fn deliver(
        &mut self,
        message: &PayloadMessage,
        to_server: &mut Sender<QueryMessage>,
    ) -> Result<()> {
        trace!("{:?}", message);
        self.agent.deliver(message, to_server)
    }
}
