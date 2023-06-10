use anyhow::{Context, Result};
use std::collections::HashSet;
use tracing::*;

use kernel::{get_my_session, EntityGid, Entry};
use plugins_core::tools;

use plugins_rpc_proto::{EntityJson, EntityKey, LookupBy, Services};

pub struct SessionServices {}

impl SessionServices {
    pub fn new_for_my_session() -> Result<Self> {
        Ok(Self {})
    }
}

impl SessionServices {
    fn lookup_one(&self, lookup: &LookupBy) -> Result<(LookupBy, Option<(Entry, EntityJson)>)> {
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
    entities: Vec<(LookupBy, Option<(Entry, EntityJson)>)>,
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
        F: FnMut(LookupBy) -> Result<(LookupBy, Option<(Entry, EntityJson)>)>,
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
    fn lookup(
        &self,
        depth: u32,
        lookup: &[LookupBy],
    ) -> Result<Vec<(LookupBy, Option<EntityJson>)>> {
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
}
