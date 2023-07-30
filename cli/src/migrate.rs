use anyhow::Result;
use clap::Args;
use plugins_rune::Behaviors;
use tracing::{debug, info};

use crate::{make_domain, PluginConfiguration};
use engine::{DevNullNotifier, SessionOpener};
use kernel::{EntityKey, HasScopes, LoadsEntities, LookupBy};

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
        }
    }

    session.close(&DevNullNotifier::default())?;

    Ok(())
}
