use anyhow::{Context, Result};
use tracing::{debug, trace};

use rpc_proto::{Payload, Query, Sender};

use crate::sessions::Services;

#[derive(Default)]
pub struct Querying {}

impl Querying {
    pub fn process(&self, messages: Vec<Query>, services: &dyn Services) -> Result<Vec<Payload>> {
        let mut payloads = Vec::new();

        for message in messages.into_iter() {
            debug!("(server) {:?}", message);

            let mut sender: Sender<Payload> = Default::default();
            self.service(&message, &mut sender, services)?;

            for payload in sender.into_iter() {
                trace!("(to-agent) {:?}", &payload);
                payloads.push(payload);
            }
        }

        Ok(payloads)
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
            Query::Raise(audience, raised) => services
                .raise(audience.clone().into(), raised.clone().into())
                .with_context(|| "raising (rpc)")?,
            Query::Schedule(key, millis, serialized) => services
                .schedule(key, *millis, serialized.clone())
                .with_context(|| "scheduling (rpc)")?,
            Query::Effect(effect) => services.produced(effect.clone().into())?,
        }

        Ok(())
    }
}

pub fn have_surroundings(
    surroundings: &kernel::prelude::Surroundings,
    services: &dyn Services,
) -> Result<Vec<Payload>> {
    let mut messages: Vec<Payload> = Vec::new();
    let keys = match &surroundings {
        kernel::prelude::Surroundings::Living {
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
        .map(|k| rpc_proto::LookupBy::Key(k.into()))
        .collect();
    const DEFAULT_DEPTH: u32 = 2;
    let resolved = services.lookup(DEFAULT_DEPTH, &lookups)?;
    for resolved in resolved {
        messages.push(Payload::Resolved(vec![resolved]));
    }

    messages.push(Payload::Surroundings(surroundings.try_into()?));

    Ok(messages)
}
