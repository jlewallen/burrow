use anyhow::Result;
use tracing::*;

use plugins_rpc_proto::{
    AlwaysErrorsServices, Inbox, Payload, Query, Sender, ServerProtocol, Services, Surroundings,
};

pub struct InProcessServer<P> {
    server: ServerProtocol,
    agent: P,
}

impl<R> InProcessServer<R>
where
    R: Default + Inbox<Payload, Query>,
{
    pub fn new() -> Self {
        Self {
            server: ServerProtocol::new(),
            agent: R::default(),
        }
    }

    pub fn initialize(&mut self) -> Result<()> {
        self.handle(Query::Bootstrap, &AlwaysErrorsServices {})
    }

    pub fn have_surroundings(
        &mut self,
        surroundings: &Surroundings,
        services: &dyn Services,
    ) -> Result<()> {
        let payload = Payload::Surroundings(surroundings.clone());
        self.send(&payload, services)
    }

    fn handle(&mut self, query: Query, services: &dyn Services) -> Result<()> {
        let mut to_server: Sender<_> = Default::default();
        to_server.send(query)?;
        self.drain(to_server, services)
    }

    fn drain(&mut self, mut to_server: Sender<Query>, services: &dyn Services) -> Result<()> {
        let mut to_agent: Sender<_> = Default::default();

        while let Some(sending) = to_server.pop() {
            self.server.apply(&sending, &mut to_agent, services)?;
            for message in to_agent.iter() {
                self.deliver(message, &mut to_server)?;
            }
        }

        Ok(())
    }

    fn send(&mut self, message: &Payload, services: &dyn Services) -> Result<()> {
        let mut to_server: Sender<_> = Default::default();

        self.deliver(message, &mut to_server)?;

        self.drain(to_server, services)
    }

    fn deliver(&mut self, message: &Payload, to_server: &mut Sender<Query>) -> Result<()> {
        trace!("{:?}", message);
        self.agent.deliver(message, to_server)
    }
}
