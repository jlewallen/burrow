use anyhow::Result;
use tracing::*;

use plugins_rpc_proto::{
    AlwaysErrorsServices, Inbox, Payload, PayloadMessage, QueryMessage, Sender, ServerProtocol,
    Services, SessionKey, Surroundings,
};

pub struct InProcessServer<P> {
    session_key: SessionKey,
    server: ServerProtocol,
    agent: P,
}

impl<R> InProcessServer<R>
where
    R: Default + Inbox<PayloadMessage, QueryMessage>,
{
    pub fn new(session_key: SessionKey) -> Self {
        Self {
            session_key: session_key.clone(),
            server: ServerProtocol::new(session_key),
            agent: R::default(),
        }
    }

    pub fn initialize(&mut self) -> Result<()> {
        self.handle(self.session_key.message(None), &AlwaysErrorsServices {})
    }

    pub fn have_surroundings(
        &mut self,
        surroundings: &Surroundings,
        services: &dyn Services,
    ) -> Result<()> {
        let payload = Payload::Surroundings(surroundings.clone());
        self.send(&self.session_key.message(payload), services)
    }

    fn handle(&mut self, query: QueryMessage, services: &dyn Services) -> Result<()> {
        let mut to_server: Sender<_> = Default::default();
        to_server.send(query)?;
        self.drain(to_server, services)
    }

    fn drain(
        &mut self,
        mut to_server: Sender<QueryMessage>,
        services: &dyn Services,
    ) -> Result<()> {
        let mut to_agent: Sender<_> = Default::default();

        while let Some(sending) = to_server.pop() {
            self.server.apply(&sending, &mut to_agent, services)?;
            for message in to_agent.iter() {
                self.deliver(message, &mut to_server)?;
            }
        }

        Ok(())
    }

    fn send(&mut self, message: &PayloadMessage, services: &dyn Services) -> Result<()> {
        let mut to_server: Sender<_> = Default::default();

        self.deliver(message, &mut to_server)?;

        self.drain(to_server, services)
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
