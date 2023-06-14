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

pub fn have_surroundings(
    surroundings: &kernel::Surroundings,
    services: &dyn Services,
) -> Result<Vec<Payload>> {
    let mut messages: Vec<Payload> = Vec::new();
    let keys = match &surroundings {
        kernel::Surroundings::Living {
            world,
            living,
            area,
        } => vec![
            world.key().clone(),
            living.key().clone(),
            area.key().clone(),
        ],
    };
    let lookups: Vec<_> = keys
        .into_iter()
        .map(|k| plugins_rpc_proto::LookupBy::Key(k.into()))
        .collect();
    const DEFAULT_DEPTH: u32 = 2;
    let resolved = services.lookup(DEFAULT_DEPTH, &lookups)?;
    for resolved in resolved {
        messages.push(Payload::Resolved(vec![resolved]));
    }

    messages.push(Payload::Surroundings(surroundings.try_into()?));

    Ok(messages)
}
