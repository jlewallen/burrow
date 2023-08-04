use anyhow::Result;

use crate::DomainBuilder;

use engine::{DevNullNotifier, HasUsernames, SessionOpener};
use kernel::{DomainError, Entry, EntryResolver, LookupBy};
use plugins_core::moving::model::Occupying;

#[tokio::main]
pub async fn execute_command() -> Result<()> {
    let builder = DomainBuilder::default();
    let domain = builder.build().await?;
    let session = domain.open_session()?;

    let world = session.world()?.expect("No world");
    let user_key = world
        .find_name_key("jlewallen")?
        .ok_or_else(|| DomainError::EntityNotFound)?;
    let user = session
        .entry(&LookupBy::Key(&user_key))?
        .expect("No 'USER' entity.");

    let occupying = user.scope::<Occupying>()?;
    let _area: Option<Entry> = occupying.area.clone().try_into()?; // TODO Annoying clone

    session.close(&DevNullNotifier {})?;

    Ok(())
}
