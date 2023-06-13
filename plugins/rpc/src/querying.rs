use anyhow::Result;

use plugins_rpc_proto::{Payload, Query, Sender};

use crate::sessions::Services;

pub struct Querying {}

impl Querying {
    pub fn new() -> Self {
        Self {}
    }

    pub fn service(
        &self,
        query: &Query,
        replies: &mut Sender<Payload>,
        _services: &dyn Services,
    ) -> Result<()> {
        match query {
            Query::Bootstrap => {
                replies.send(Payload::Initialize)?;
            }
            Query::Complete => {}
            Query::Update(_) => todo!(),
            Query::Raise(_) => todo!(),
            Query::Chain(_) => todo!(),
            Query::Reply(_) => todo!(),
            Query::Permission(_) => todo!(),
            Query::Lookup(_, _) => todo!(),
            Query::Find(_) => todo!(),
            Query::Try(_) => todo!(),
        }

        Ok(())
    }
}
