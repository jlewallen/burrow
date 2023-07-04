use anyhow::{anyhow, Context, Result};
use std::collections::HashSet;
use tracing::*;

use kernel::{
    deserialize_entity_from_value, get_my_session, Audience, DomainError, DomainEvent, EntityGid,
    Entry, Observed, ToJson,
};
use plugins_core::tools;

use plugins_rpc_proto::{EntityKey, EntityUpdate, Json, LookupBy};

pub trait Services {
    fn lookup(&self, depth: u32, lookup: &[LookupBy]) -> Result<Vec<(LookupBy, Option<Json>)>>;

    fn apply_update(&self, update: EntityUpdate) -> Result<()>;

    fn raise(&self, audience: Audience, raised: serde_json::Value) -> Result<()>;
}

pub struct AlwaysErrorsServices {}

impl Services for AlwaysErrorsServices {
    fn lookup(&self, _depth: u32, _lookup: &[LookupBy]) -> Result<Vec<(LookupBy, Option<Json>)>> {
        warn!("AlwaysErrorsServices::lookup");
        Err(anyhow!("This server always errors (lookup)"))
    }

    fn apply_update(&self, _update: EntityUpdate) -> Result<()> {
        warn!("AlwaysErrorsServices::update");
        Err(anyhow!("This server always errors (apply_update)"))
    }

    fn raise(&self, _audience: Audience, _raised: serde_json::Value) -> Result<()> {
        warn!("AlwaysErrorsServices::raise");
        Err(anyhow!("This server always errors (raise)"))
    }
}

pub struct SessionServices {}

impl SessionServices {
    pub fn new_for_my_session() -> Result<Self> {
        Ok(Self {})
    }

    fn lookup_one(&self, lookup: &LookupBy) -> Result<(LookupBy, Option<(Entry, Json)>)> {
        let session = get_my_session().with_context(|| "SessionServer::lookup_one")?;
        let entry = match lookup {
            LookupBy::Key(key) => session.entry(&kernel::LookupBy::Key(&key.into()))?,
            LookupBy::Gid(gid) => session.entry(&kernel::LookupBy::Gid(&EntityGid::new(*gid)))?,
        };

        match entry {
            Some(entry) => Ok((lookup.clone(), Some((entry.clone(), (&entry).try_into()?)))),
            None => Ok((lookup.clone(), None)),
        }
    }
}

#[derive(Default)]
struct FoldToDepth {
    queue: Vec<LookupBy>,
    entities: Vec<(LookupBy, Option<(Entry, Json)>)>,
}

impl FoldToDepth {
    pub fn new(prime: &[LookupBy]) -> Self {
        Self {
            queue: prime.into(),
            ..Default::default()
        }
    }

    pub fn into_with<F>(self, f: F) -> Result<Self>
    where
        F: FnMut(LookupBy) -> Result<(LookupBy, Option<(Entry, Json)>)>,
    {
        debug!(queue = self.queue.len(), "discovering");

        let have: HashSet<&kernel::EntityKey> = self
            .entities
            .iter()
            .filter_map(|(_lookup, maybe)| maybe.as_ref().map(|m| m.0.key()))
            .collect();

        let adding = self.queue.into_iter().map(f).collect::<Result<Vec<_>>>()?;

        let queue = adding
            .iter()
            .map(|(_lookup, maybe)| match maybe {
                Some((entry, _)) => {
                    let mut keys = Vec::new();
                    keys.extend(tools::get_contained_keys(entry)?);
                    keys.extend(tools::get_occupant_keys(entry)?);
                    keys.extend(tools::get_adjacent_keys(entry)?);
                    Ok(keys)
                }
                None => Ok(vec![]),
            })
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flat_map(|v| v.into_iter())
            .collect::<HashSet<kernel::EntityKey>>()
            .into_iter()
            .filter_map(|key| have.get(&key).map_or(Some(key), |_| None))
            .map(|key| LookupBy::Key(EntityKey::new(key.to_string())))
            .collect();

        let entities = self.entities.into_iter().chain(adding).collect();

        Ok(Self { queue, entities })
    }
}

impl Services for SessionServices {
    fn lookup(&self, depth: u32, lookup: &[LookupBy]) -> Result<Vec<(LookupBy, Option<Json>)>> {
        let done = (0..depth).fold(
            Ok::<_, anyhow::Error>(FoldToDepth::new(lookup)),
            |acc, depth| match acc {
                Ok(acc) => {
                    let _span = span!(Level::INFO, "folding", depth = depth).entered();
                    acc.into_with(|lookup| self.lookup_one(&lookup))
                }
                Err(e) => Err(e),
            },
        )?;

        info!(nentities = done.entities.len(), depth = depth, "lookup");

        Ok(done
            .entities
            .into_iter()
            .map(|(lookup, maybe)| (lookup, maybe.map(|m| m.1)))
            .collect())
    }

    fn apply_update(&self, update: EntityUpdate) -> Result<()> {
        let session = get_my_session().with_context(|| "SessionServer::apply_update")?;

        if let Some(entry) = session.entry(&kernel::LookupBy::Key(&update.key.into()))? {
            let value: serde_json::Value = update.entity.into();
            let replacing = deserialize_entity_from_value(value)?;
            let entity = entry.entity()?;
            entity.replace(replacing);
            Ok(())
        } else {
            Err(anyhow!("Updating (adding?) missing entity."))
        }
    }

    fn raise(&self, audience: Audience, raised: serde_json::Value) -> Result<()> {
        let session = get_my_session().with_context(|| "SessionServer::raise")?;
        session.raise(audience, Box::new(RpcDomainEvent { value: raised }))
    }
}

#[derive(Debug)]
pub struct RpcDomainEvent {
    value: serde_json::Value,
}

impl DomainEvent for RpcDomainEvent {
    fn observe(&self, _user: &Entry) -> Result<Box<dyn kernel::Observed>, DomainError> {
        Ok(Box::new(RpcDomainEvent {
            value: self.value.clone(),
        }))
    }
}

impl ToJson for RpcDomainEvent {
    fn to_json(&self) -> std::result::Result<serde_json::Value, serde_json::Error> {
        Ok(self.value.clone())
    }
}

impl Observed for RpcDomainEvent {}
