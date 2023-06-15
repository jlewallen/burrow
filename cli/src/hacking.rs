use anyhow::Result;
use tracing::info;

use crate::text::Renderer;
use crate::{make_domain, PluginConfiguration};

use engine::{username_to_key, DevNullNotifier, SessionOpener};
use kernel::{ActiveSession, DomainError, Entry, LookupBy};
use plugins_core::carrying::model::{Carryable, Containing};
use plugins_core::moving::model::Occupying;

pub fn set_containing_quantities_to_1(thing: Entry) -> Result<()> {
    let holding = thing
        .scope::<Containing>()?
        .holding
        .iter()
        .map(|i| -> Result<Option<Entry>> { Ok(i.clone().try_into()?) }) // TODO Annoying clone
        .collect::<Result<Vec<_>>>()?;

    for held in holding.iter() {
        if let Some(item) = &held {
            let mut carryable = item.scope_mut::<Carryable>()?;
            info!("{:?} quantity = {}", item, carryable.quantity());
            carryable.set_quantity(1.0)?;
            carryable.save()?;
        }
    }

    Ok(())
}

#[tokio::main]
pub async fn execute_command() -> Result<()> {
    let _renderer = Renderer::new()?;
    let domain = make_domain(PluginConfiguration::default()).await?;
    let session = domain.open_session()?;

    let world = session.world()?;
    let user_key =
        username_to_key(&world, "jlewallen")?.ok_or_else(|| DomainError::EntityNotFound)?;
    let user = session
        .entry(&LookupBy::Key(&user_key))?
        .expect("No 'USER' entity.");

    let occupying = user.scope::<Occupying>()?;
    let area: Option<Entry> = occupying.area.clone().try_into()?; // TODO Annoying clone

    if let Some(area) = area {
        set_containing_quantities_to_1(area)?;
    }

    set_containing_quantities_to_1(user)?;

    session.close(&DevNullNotifier {})?;

    Ok(())
}
