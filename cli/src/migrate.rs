use anyhow::Result;
use clap::Args;
use plugins_core::{
    carrying::model::{Carryable, Containing},
    fashion::model::{Wearable, Wearing},
    location::Location,
    moving::model::Occupyable,
};
use plugins_rune::Behaviors;
use tracing::{debug, info};

use crate::{make_domain, PluginConfiguration};
use engine::{DevNullNotifier, SessionOpener};
use kernel::{DomainError, EntityKey, Entry, HasScopes, LoadsEntities, LookupBy, Scope};

#[derive(Debug, Args, Clone)]
pub struct Command {}

impl Command {
    fn plugin_configuration(&self) -> PluginConfiguration {
        PluginConfiguration {
            wasm: false,
            dynlib: false,
            rune: false,
            rpc: false,
        }
    }
}

fn load_and_save_scope<T: Scope>(entity: &Entry) -> Result<bool, DomainError> {
    if entity.has_scope::<T>()? {
        Ok(entity.scope_mut::<T>()?.save().map(|_| true)?)
    } else {
        Ok(false)
    }
}

#[tokio::main]
pub async fn execute_command(cmd: &Command) -> Result<()> {
    let domain = make_domain(cmd.plugin_configuration()).await?;

    info!("loading keys...");
    let entities = domain.query_all()?;
    let keys: Vec<EntityKey> = entities
        .into_iter()
        .map(|e| EntityKey::new(&e.key))
        .collect();

    info!("have {} keys", keys.len());
    let session = domain.open_session()?;

    for key in keys.iter() {
        info!("loading {:?}", key);
        let entity = session.load_entity(&LookupBy::Key(key))?;
        if let Some(entity) = entity {
            debug!("{:?}", entity.key());

            if false {
                // Reset Behaviors scope on all entities.
                let mut entity = entity.borrow_mut();
                let mut scopes = entity.scopes_mut();
                scopes.replace_scope(&Behaviors::default())?;
            }
            if true {
                let entry: Entry = entity.try_into()?;
                load_and_save_scope::<Location>(&entry)?;
                load_and_save_scope::<Carryable>(&entry)?;
                load_and_save_scope::<Occupyable>(&entry)?;
                load_and_save_scope::<Containing>(&entry)?;
                load_and_save_scope::<Wearable>(&entry)?;
                load_and_save_scope::<Wearing>(&entry)?;
            }
        }
    }

    session.close(&DevNullNotifier::default())?;

    Ok(())
}
