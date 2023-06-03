#[cfg(test)]
mod tests {
    use std::{rc::Rc, sync::Arc};

    use anyhow::Result;
    use engine::{
        storage, DevNullNotifier, Domain, EntityStorageFactory, Finder, Session, SessionOpener,
    };
    use kernel::RegisteredPlugins;
    use plugins_core::{
        building::BuildingPluginFactory, carrying::CarryingPluginFactory,
        dynamic::DynamicPluginFactory, looking::LookingPluginFactory, moving::MovingPluginFactory,
        BuildSurroundings, DefaultFinder, QuickThing,
    };
    use plugins_rpc::RpcPluginFactory;
    use plugins_rune::RunePluginFactory;
    use plugins_wasm::WasmPluginFactory;
    use tokio::{runtime::Handle, task::JoinHandle};

    #[derive(Clone)]
    struct AsyncFriendlyDomain {
        domain: Domain,
    }

    impl AsyncFriendlyDomain {
        pub fn new(
            storage_factory: Arc<dyn EntityStorageFactory>,
            plugins: Arc<RegisteredPlugins>,
            finder: Arc<dyn Finder>,
            deterministic: bool,
        ) -> Self {
            Self {
                domain: Domain::new(storage_factory, plugins, finder, deterministic),
            }
        }

        pub async fn evaluate<W>(&self, text: &'static [&'static str]) -> Result<()>
        where
            W: WorldFixture + Default,
        {
            let handle: JoinHandle<Result<()>> = tokio::task::spawn_blocking({
                let sessions = self.clone();
                move || {
                    let session = sessions.open_session()?;

                    let fixture = W::default();

                    fixture.prepare(&session)?;

                    for text in text {
                        if let Some(reply) = session.evaluate_and_perform("burrow", text)? {
                            println!("{:?}", &reply);
                        }
                    }

                    session.close(&DevNullNotifier::default())?;

                    Ok(())
                }
            });

            Ok(handle.await??)
        }

        pub async fn stop(&self) -> Result<()> {
            let domain = self.domain.clone();
            tokio::task::spawn_blocking(move || domain.stop()).await?
        }
    }

    impl SessionOpener for AsyncFriendlyDomain {
        fn open_session(&self) -> Result<std::rc::Rc<Session>> {
            self.domain.open_session()
        }
    }

    async fn test_domain() -> Result<AsyncFriendlyDomain> {
        let storage_factory = storage::sqlite::Factory::new(storage::sqlite::MEMORY_SPECIAL)?;
        let mut registered_plugins = RegisteredPlugins::default();
        registered_plugins.register(MovingPluginFactory::default());
        registered_plugins.register(LookingPluginFactory::default());
        registered_plugins.register(CarryingPluginFactory::default());
        registered_plugins.register(BuildingPluginFactory::default());
        registered_plugins.register(DynamicPluginFactory::default());
        registered_plugins.register(RunePluginFactory::default());
        registered_plugins.register(WasmPluginFactory::default());
        registered_plugins.register(RpcPluginFactory::start(Handle::current()).await?);
        let finder = Arc::new(DefaultFinder::default());
        Ok(AsyncFriendlyDomain::new(
            storage_factory,
            Arc::new(registered_plugins),
            finder,
            true,
        ))
    }

    trait WorldFixture {
        fn prepare(&self, session: &Rc<Session>) -> Result<()>;
    }

    #[derive(Default)]
    struct Noop {}

    impl WorldFixture for Noop {
        fn prepare(&self, _session: &Rc<Session>) -> Result<()> {
            Ok(())
        }
    }

    #[derive(Default)]
    struct KeyInVessel {}

    impl WorldFixture for KeyInVessel {
        fn prepare(&self, session: &Rc<Session>) -> Result<()> {
            let mut build = BuildSurroundings::new_in_session(session.clone())?;
            let key = build.entity()?.named("Key")?.into_entry()?;
            let vessel = build
                .entity()?
                .named("Vessel")?
                .holding(&vec![key.clone()])?
                .into_entry()?;
            let (_session, _surroundings) = build
                .hands(vec![QuickThing::Actual(vessel.clone())])
                .build()?;

            session.flush()?;

            Ok(())
        }
    }

    #[tokio::test]
    async fn it_evaluates_a_simple_look() -> Result<()> {
        let domain = test_domain().await?;
        domain.evaluate::<KeyInVessel>(&["look"]).await?;
        domain.stop().await?;

        Ok(())
    }

    #[tokio::test]
    async fn it_evaluates_two_simple_looks_same_session() -> Result<()> {
        let domain = test_domain().await?;
        domain.evaluate::<KeyInVessel>(&["look", "look"]).await?;
        domain.stop().await?;

        Ok(())
    }

    #[tokio::test]
    async fn it_evaluates_two_simple_looks_separate_session() -> Result<()> {
        let domain = test_domain().await?;
        domain.evaluate::<KeyInVessel>(&["look"]).await?;
        domain.evaluate::<Noop>(&["look"]).await?;
        domain.stop().await?;

        Ok(())
    }
}

#[cfg(test)]
#[ctor::ctor]
fn initialize_tests() {
    plugins_core::log_test();
}
