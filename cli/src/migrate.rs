use anyhow::Result;
use clap::Args;
use plugins_core::{
    carrying::model::{Carryable, Containing},
    fashion::model::{Wearable, Wearing},
    location::Location,
    memory::model::Memory,
    moving::model::{Exit, Occupyable, Occupying, Route, SimpleRoute},
    tools,
};
use plugins_rune::Behaviors;
use tracing::info;

use crate::DomainBuilder;
use engine::{
    prelude::DevNullNotifier,
    prelude::{HasWellKnownEntities, SessionOpener},
    storage::StorageFactory,
};
use kernel::prelude::{
    DomainError, DomainOutcome, EntityKey, EntityPtr, EntityPtrResolver, IntoEntityPtr,
    LoadAndStoreScope, LookupBy, OpenScope, OpenScopeRefMut, Properties, Scope,
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
    #[arg(long)]
    routes: bool,
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

    if cmd.scopes || cmd.scope.is_some() || cmd.routes {
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

            let entity = session.entity(&LookupBy::Key(key))?;
            if let Some(entity) = entity {
                if cmd.routes {
                    if let Some(containing) = entity.scope::<Containing>()? {
                        let mut occupyable = entity.scope_mut::<Occupyable>()?;
                        for item in containing.holding.iter() {
                            let item = item.to_entity()?;
                            if let Some(exit) = item.scope::<Exit>()? {
                                let world = session.world()?.unwrap();
                                let limbo_key = world.get_limbo()?.unwrap();
                                let limbo = session.entity(&LookupBy::Key(&limbo_key))?.unwrap();

                                if limbo_key != entity.key() {
                                    info!("found legacy exit {:?}", item.name()?.unwrap());

                                    let simple =
                                        SimpleRoute::new(&item.name()?.unwrap(), exit.area.clone());
                                    occupyable.add_route(Route::Simple(simple))?;
                                    occupyable.save()?;

                                    {
                                        let mut item = item.borrow_mut();
                                        item.remove_scope(Carryable::scope_key());
                                    }

                                    assert_eq!(
                                        tools::move_between(&entity, &limbo, &item)?,
                                        DomainOutcome::Ok
                                    );
                                }
                            }
                        }
                    }
                }

                if let Some(key) = &cmd.scope {
                    let mut entity = entity.borrow_mut();
                    if cmd.erase {
                        entity.remove_scope(key);
                    } else if let Some(new_key) = &cmd.rename {
                        entity.rename_scope(key, new_key);
                    }
                }

                if cmd.scopes {
                    load_and_save_scope::<Properties>(&entity)?;
                    load_and_save_scope::<Location>(&entity)?;
                    load_and_save_scope::<Carryable>(&entity)?;
                    load_and_save_scope::<Occupyable>(&entity)?;
                    load_and_save_scope::<Occupying>(&entity)?;
                    load_and_save_scope::<Exit>(&entity)?;
                    load_and_save_scope::<Containing>(&entity)?;
                    load_and_save_scope::<Wearable>(&entity)?;
                    load_and_save_scope::<Wearing>(&entity)?;
                    load_and_save_scope::<Memory>(&entity)?;
                    load_and_save_scope::<Behaviors>(&entity)?;
                }
            }
        }

        session.close(&DevNullNotifier::default())?;
    }

    Ok(())
}
