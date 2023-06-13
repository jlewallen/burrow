use anyhow::Result;

use plugins_rpc_proto::{Inbox, Payload, Query, Sender};

#[derive(Debug)]
pub struct ExampleAgent {}

impl Default for ExampleAgent {
    fn default() -> Self {
        Self::new()
    }
}

impl ExampleAgent {
    pub fn new() -> Self {
        Self {}
    }
}

impl Inbox<Payload, Query> for ExampleAgent {
    fn deliver(&mut self, message: &Payload, replies: &mut Sender<Query>) -> Result<()> {
        match message {
            Payload::Initialize => {}
            Payload::Surroundings(_) => replies.send(Query::Complete)?,
            Payload::Evaluate(_, _) => todo!(),
            Payload::Resolved(_) => todo!(),
            Payload::Found(_) => todo!(),
            Payload::Permission(_) => todo!(),
            Payload::Hook(_) => todo!(),
        }

        Ok(())
    }
}
