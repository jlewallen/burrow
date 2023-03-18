use anyhow::Result;
use tracing::info;

use crate::domain::{domain, DevNullNotifier};
use crate::kernel::Entry;
use crate::plugins::carrying::model::{Carryable, Containing};
use crate::plugins::moving::model::Occupying;
use crate::plugins::users::model::Usernames;
use crate::storage;
use crate::text::Renderer;

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

pub fn execute_command() -> Result<()> {
    let _renderer = Renderer::new()?;
    let storage_factory = storage::sqlite::Factory::new("world.sqlite3")?;
    let domain = domain::Domain::new(storage_factory, false);
    let session = domain.open_session()?;

    let world = session.world()?;
    let usernames = world.scope::<Usernames>()?;
    let user_key = &usernames.users["jlewallen"];
    let user = session.entry(user_key)?.expect("No 'USER' entity.");

    let occupying = user.scope::<Occupying>()?;
    let area: Option<Entry> = occupying.area.clone().try_into()?; // TODO Annoying clone

    if let Some(area) = area {
        set_containing_quantities_to_1(area)?;
    }

    set_containing_quantities_to_1(user)?;

    session.close(&DevNullNotifier {})?;

    Ok(())
}
