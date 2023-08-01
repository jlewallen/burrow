use anyhow::Result;
use plugins_core::chat::ChatPluginFactory;
use plugins_core::emote::EmotePluginFactory;
use plugins_core::memory::MemoryPluginFactory;
use std::env::temp_dir;
use std::rc::Rc;
use std::sync::Arc;

use engine::storage::{InMemoryStorageFactory, StorageFactory};
use engine::{sequences::DeterministicKeys, DevNullNotifier, Domain, Session, SessionOpener};
use kernel::RegisteredPlugins;
use plugins_core::building::BuildingPluginFactory;
use plugins_core::carrying::CarryingPluginFactory;
use plugins_core::looking::LookingPluginFactory;
use plugins_core::moving::MovingPluginFactory;
use plugins_core::DefaultFinder;
use plugins_core::{BuildSurroundings, QuickThing};
use plugins_dynlib::DynamicPluginFactory;
use plugins_rune::RunePluginFactory;
use plugins_wasm::WasmPluginFactory;
use replies::Reply;

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

        session.flush(&DevNullNotifier {})?;

        Ok(())
    }
}

fn evaluate_fixture<W, S>(
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

fn make_domain() -> Result<Domain> {
    test_domain_with(InMemoryStorageFactory::default())
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

pub fn test_domain_with<S>(storage: S) -> Result<Domain>
where
    S: StorageFactory + 'static,
{
    let storage_factory = Arc::new(storage);
    let mut registered_plugins = RegisteredPlugins::default();
    if false {
        registered_plugins.register(RunePluginFactory::default());
        registered_plugins.register(WasmPluginFactory::new(&temp_dir())?);
    }
    registered_plugins.register(DynamicPluginFactory::default());
    registered_plugins.register(LookingPluginFactory::default());
    registered_plugins.register(ChatPluginFactory::default());
    registered_plugins.register(EmotePluginFactory::default());
    registered_plugins.register(MovingPluginFactory::default());
    registered_plugins.register(CarryingPluginFactory::default());
    registered_plugins.register(BuildingPluginFactory::default());
    registered_plugins.register(MemoryPluginFactory::default());
    let finder = Arc::new(DefaultFinder::default());
    let keys = Arc::new(DeterministicKeys::new());
    let identities = Arc::new(DeterministicKeys::new());
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
