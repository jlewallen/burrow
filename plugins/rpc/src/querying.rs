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
        services: &dyn Services,
    ) -> Result<()> {
        match query {
            Query::Complete => {}
            Query::Bootstrap => replies.send(Payload::Initialize)?,
            Query::Update(update) => services.apply_update(update.clone())?,
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
