use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};
use tracing::*;

use kernel::common::identifier_to_key;
use kernel::prelude::*;
use macros::*;
use plugins_core::tools;
use rpc_proto::{EntityKey, EntityUpdate, FutureSchedule, Json, LookupBy};

pub trait Services {
    fn lookup(&self, depth: u32, lookup: &[LookupBy]) -> Result<Vec<(LookupBy, Option<Json>)>>;

    fn apply_update(&self, update: EntityUpdate) -> Result<()>;

    fn raise(&self, actor: Option<EntityPtr>, audience: Audience, raised: JsonValue) -> Result<()>;

    fn schedule(
        &self,
        key: &str,
        entity: EntityKey,
        schedule: FutureSchedule,
        serialized: Json,
    ) -> Result<()>;

    fn produced(&self, effect: Effect) -> Result<()>;
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

    fn raise(
        &self,
        _actor: Option<EntityPtr>,
        _audience: Audience,
        _raised: JsonValue,
    ) -> Result<()> {
        warn!("AlwaysErrorsServices::raise");
        Err(anyhow!("This server always errors (raise)"))
    }

    fn schedule(
        &self,
        _key: &str,
        _entity: EntityKey,
        _schedule: FutureSchedule,
        _serialized: Json,
    ) -> Result<()> {
        warn!("AlwaysErrorsServices::schedule");
        Err(anyhow!("This server always errors (schedule)"))
    }

    fn produced(&self, _effect: Effect) -> Result<()> {
        warn!("AlwaysErrorsServices::produced");
        Err(anyhow!("This server always errors (produced)"))
    }
}

pub struct SessionServices {
    prefix: Option<String>,
    produced: Arc<Mutex<Option<Vec<Effect>>>>,
}

impl SessionServices {
    pub fn new_for_my_session(prefix: Option<&str>) -> Result<Self> {
        Ok(Self {
            prefix: prefix.map(|s| s.to_owned()),
            produced: Default::default(),
        })
    }

    pub fn take_produced(&self) -> Result<Option<Vec<Effect>>> {
        let mut produced = self.produced.lock().expect("Lock error");

        Ok(produced.take())
    }

    fn lookup_one(&self, lookup: &LookupBy) -> Result<(LookupBy, Option<(EntityPtr, Json)>)> {
        let session = get_my_session().with_context(|| "SessionServer::lookup_one")?;
        let entity = match lookup {
            LookupBy::Key(key) => session.entity(&kernel::prelude::LookupBy::Key(&key.into()))?,
            LookupBy::Gid(gid) => {
                session.entity(&kernel::prelude::LookupBy::Gid(&EntityGid::new(*gid)))?
            }
        };

        match entity {
            Some(entity) => Ok((
                lookup.clone(),
                Some((entity.clone(), (&entity).try_into()?)),
            )),
            None => Ok((lookup.clone(), None)),
        }
    }
}

#[derive(Default)]
struct FoldToDepth {
    queue: Vec<LookupBy>,
    entities: Vec<(LookupBy, Option<(EntityPtr, Json)>)>,
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
        F: FnMut(LookupBy) -> Result<(LookupBy, Option<(EntityPtr, Json)>)>,
    {
        debug!(queue = self.queue.len(), "discovering");

        let have: HashSet<kernel::prelude::EntityKey> = self
            .entities
            .iter()
            .filter_map(|(_lookup, maybe)| maybe.as_ref().map(|m| m.0.key()))
            .collect();

        let adding = self.queue.into_iter().map(f).collect::<Result<Vec<_>>>()?;

        let queue = adding
            .iter()
            .map(|(_lookup, maybe)| match maybe {
                Some((entity, _)) => {
                    let mut keys = Vec::new();
                    keys.extend(tools::get_contained_keys(entity)?);
                    keys.extend(tools::get_occupant_keys(entity)?);
                    keys.extend(tools::get_adjacent_keys(entity)?);
                    Ok(keys)
                }
                None => Ok(vec![]),
            })
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flat_map(|v| v.into_iter())
            .collect::<HashSet<kernel::prelude::EntityKey>>()
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

        if let Some(entity) = session.entity(&kernel::prelude::LookupBy::Key(&update.key.into()))? {
            let value: JsonValue = update.entity.into();
            let replacing = Entity::from_value(value)?;
            entity.replace(replacing);
            Ok(())
        } else {
            Err(anyhow!("Updating (adding?) missing entity."))
        }
    }

    fn raise(&self, actor: Option<EntityPtr>, audience: Audience, raised: JsonValue) -> Result<()> {
        let session = get_my_session().with_context(|| "SessionServer::raise")?;
        Ok(session.raise(
            actor,
            audience,
            Raising::TaggedJson(RpcDomainEvent { value: raised }.to_tagged_json()?),
        )?)
    }

    fn schedule(
        &self,
        key: &str,
        entity: EntityKey,
        schedule: FutureSchedule,
        serialized: Json,
    ) -> Result<()> {
        let session = get_my_session().with_context(|| "SessionServer::schedule")?;
        let prefix = self
            .prefix
            .as_ref()
            .ok_or_else(|| anyhow!("session prefix required"))?;

        let serialized: JsonValue = serialized.into();

        Ok(session.schedule(FutureAction::new(
            format!("{}/{}", prefix, key),
            entity.into(),
            schedule.into(),
            serialized.try_into()?,
        ))?)
    }

    fn produced(&self, effect: Effect) -> Result<()> {
        let mut produced = self.produced.lock().expect("Lock error");

        if produced.is_none() {
            *produced = Some(vec![effect]);
        } else {
            produced.as_mut().unwrap().push(effect);
        }

        Ok(())
    }
}

#[derive(Debug, Serialize, ToTaggedJson)]
pub struct RpcDomainEvent {
    value: JsonValue,
}

impl DomainEvent for RpcDomainEvent {}
