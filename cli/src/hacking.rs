use anyhow::Result;

use engine::prelude::{DevNullNotifier, HasUsernames, SessionOpener};
use kernel::{
    here,
    prelude::{DomainError, EntryResolver, LookupBy},
};

use crate::DomainBuilder;

#[tokio::main]
pub async fn execute_command() -> Result<()> {
    let builder = DomainBuilder::default();
    let domain = builder.build().await?;
    let session = domain.open_session()?;

    let world = session.world()?.expect("No world");
    let user_key = world
        .find_name_key("jlewallen")?
        .ok_or(DomainError::EntityNotFound(here!().into()))?;
    let _user = session
        .entry(&LookupBy::Key(&user_key))?
        .expect("No 'USER' entity.");

    // let occupying = user.scope::<Occupying>()?.unwrap();
    // let _area: Option<Entry> = occupying.area.clone().try_into()?; // TODO Annoying clone

    session.close(&DevNullNotifier {})?;

    Ok(())
}
