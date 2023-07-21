use anyhow::Result;
use replies::Reply;
use std::rc::Rc;
use std::sync::Arc;

use engine::sequences::DeterministicKeys;
use engine::Domain;
use engine::{DevNullNotifier, Session, SessionOpener};
use kernel::RegisteredPlugins;
use plugins_core::building::BuildingPluginFactory;
use plugins_core::carrying::CarryingPluginFactory;
use plugins_core::looking::LookingPluginFactory;
use plugins_core::moving::MovingPluginFactory;
use plugins_core::DefaultFinder;
use plugins_core::{BuildSurroundings, QuickThing};
use sqlite::Factory;

pub const USERNAME: &str = "burrow";

pub trait WorldFixture {
    fn prepare(&self, session: &Rc<Session>) -> Result<()>;
}

#[derive(Default)]
pub struct Noop {}

impl WorldFixture for Noop {
    fn prepare(&self, _session: &Rc<Session>) -> Result<()> {
        Ok(())
    }
}

#[derive(Default)]
pub struct HoldingKeyInVessel {}

impl WorldFixture for HoldingKeyInVessel {
    fn prepare(&self, session: &Rc<Session>) -> Result<()> {
        let mut build = BuildSurroundings::new_in_session(session.clone())?;

        let place = build.make(QuickThing::Place("Place"))?;
        let key = build.entity()?.named("Key")?.into_entry()?;
        let vessel = build
            .entity()?
            .named("Vessel")?
            .holding(&vec![key.clone()])?
            .into_entry()?;
        let (_, _surroundings) = build
            .route("East", QuickThing::Actual(place.clone()))
            .hands(vec![QuickThing::Actual(vessel.clone())])
            .build()?;

        session.flush()?;

        Ok(())
    }
}

pub fn evaluate_fixture<W, S>(
    domain: &S,
    username: &str,
    text: &'static [&'static str],
) -> Result<Option<Box<dyn Reply>>>
where
    W: WorldFixture + Default,
    S: SessionOpener,
{
    let session = domain.open_session()?;

    let fixture = W::default();

    fixture.prepare(&session)?;

    for text in text {
        if let Some(_reply) = session.evaluate_and_perform(username, text)? {
            // Do nothing, for now.
        }
    }

    session.close(&DevNullNotifier::default())?;

    Ok(None)
}

pub fn evaluate_text_in_new_domain<W>(
    username: &str,
    times: usize,
    text: &'static [&'static str],
) -> Result<()>
where
    W: WorldFixture + Default,
{
    let domain = make_domain()?;

    assert!(times > 0);

    evaluate_fixture::<W, _>(&domain, username, text)?;

    for _ in [0..times - 1] {
        evaluate_fixture::<Noop, _>(&domain, username, text)?;
    }

    Ok(())
}

pub fn make_domain() -> Result<Domain> {
    let storage_factory = Factory::new(":memory:")?;
    let mut registered_plugins = RegisteredPlugins::default();
    registered_plugins.register(LookingPluginFactory::default());
    registered_plugins.register(MovingPluginFactory::default());
    registered_plugins.register(CarryingPluginFactory::default());
    registered_plugins.register(BuildingPluginFactory::default());
    let keys = Arc::new(DeterministicKeys::new());
    let identities = Arc::new(DeterministicKeys::new());
    let finder = Arc::new(DefaultFinder::default());
    Ok(Domain::new(
        storage_factory,
        Arc::new(registered_plugins),
        finder,
        keys,
        identities,
    ))
}

#[cfg(test)]
mod tests;
