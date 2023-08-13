use anyhow::Result;
use clap::Args;
use plugins_core::{
    carrying::model::{Carryable, Containing},
    fashion::model::{Wearable, Wearing},
    location::Location,
    memory::model::Memory,
    moving::model::{Exit, Movement, Occupyable, Occupying},
};
use plugins_rune::Behaviors;
use tracing::info;

use crate::DomainBuilder;
use engine::{prelude::DevNullNotifier, prelude::SessionOpener, storage::StorageFactory};
use kernel::prelude::{
    DomainError, EntityKey, EntityPtr, EntityPtrResolver, LoadAndStoreScope, LookupBy, OpenScope,
    OpenScopeRefMut, Properties, Scope,
};

#[derive(Debug, Args, Clone)]
pub struct Command {
    #[arg(short, long, value_name = "FILE")]
    path: Option<String>,
    #[arg(long)]
    scopes: bool,
    #[arg(long)]
    scope: Option<String>,
    #[arg(long)]
    erase: bool,
    #[arg(long)]
    rename: Option<String>,
}

impl Command {
    fn builder(&self) -> DomainBuilder {
        DomainBuilder::new(self.path.clone())
    }
}

fn load_and_save_scope<T: Scope>(entity: &EntityPtr) -> Result<bool, DomainError> {
    use anyhow::Context;
    if entity.scope::<T>()?.is_some() {
        tracing::trace!("{:?} {:?}", entity.key(), T::scope_key());

        Ok(entity
            .scope_mut::<T>()
            .with_context(|| T::scope_key().to_string())?
            .save()
            .map(|_| true)?)
    } else {
        Ok(false)
    }
}

#[tokio::main]
pub async fn execute_command(cmd: &Command) -> Result<()> {
    let builder = cmd.builder();
    let domain = builder.build().await?;

    let factory = builder.storage_factory()?;
    let storage = factory.create_storage()?;

    if cmd.scopes || cmd.scope.is_some() {
        info!("loading keys...");

        let entities = storage.query_all()?;
        let keys: Vec<EntityKey> = entities
            .into_iter()
            .map(|e| EntityKey::new(&e.key))
            .collect();

        let session = domain.open_session()?;
        let session = session.set_session()?;

        for key in keys.iter() {
            info!("processing {:?}", key);

            let entry = session.entry(&LookupBy::Key(key))?;
            if let Some(entry) = entry {
                let entity = entry.entity();

                if let Some(key) = &cmd.scope {
                    let mut entity = entity.borrow_mut();
                    if cmd.erase {
                        entity.remove_scope(key);
                    } else if let Some(new_key) = &cmd.rename {
                        entity.rename_scope(key, new_key);
                    }
                }

                if cmd.scopes {
                    load_and_save_scope::<Properties>(&entry)?;
                    load_and_save_scope::<Location>(&entry)?;
                    load_and_save_scope::<Carryable>(&entry)?;
                    load_and_save_scope::<Occupyable>(&entry)?;
                    load_and_save_scope::<Occupying>(&entry)?;
                    load_and_save_scope::<Exit>(&entry)?;
                    load_and_save_scope::<Movement>(&entry)?;
                    load_and_save_scope::<Containing>(&entry)?;
                    load_and_save_scope::<Wearable>(&entry)?;
                    load_and_save_scope::<Wearing>(&entry)?;
                    load_and_save_scope::<Memory>(&entry)?;
                    load_and_save_scope::<Behaviors>(&entry)?;
                }
            }
        }

        session.close(&DevNullNotifier::default())?;
    }

    Ok(())
}
