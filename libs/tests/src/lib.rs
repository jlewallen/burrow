#[cfg(test)]
mod tests {
    use anyhow::Result;
    use std::{rc::Rc, sync::Arc};
    use tokio::task::JoinHandle;

    use engine::{
        sequences::{DeterministicKeys, Sequence},
        storage::EntityStorageFactory,
        storage::PersistedEntity,
        DevNullNotifier, Domain, Session, SessionOpener,
    };
    use kernel::{EntityKey, Finder, Identity, RegisteredPlugins};
    use plugins_core::{
        building::BuildingPluginFactory, carrying::CarryingPluginFactory,
        looking::LookingPluginFactory, moving::MovingPluginFactory, BuildSurroundings,
        DefaultFinder, QuickThing,
    };
    use plugins_dynlib::DynamicPluginFactory;
    use plugins_rpc::RpcPluginFactory;
    use plugins_rune::RunePluginFactory;
    use plugins_wasm::WasmPluginFactory;

    const USERNAME: &str = "burrow";

    #[derive(Clone)]
    struct AsyncFriendlyDomain {
        domain: Domain,
    }

    impl AsyncFriendlyDomain {
        pub fn new<SF>(
            storage_factory: Arc<SF>,
            plugins: Arc<RegisteredPlugins>,
            finder: Arc<dyn Finder>,
            keys: Arc<dyn Sequence<EntityKey>>,
            identities: Arc<dyn Sequence<Identity>>,
        ) -> Self
        where
            SF: EntityStorageFactory + 'static,
        {
            Self {
                domain: Domain::new(storage_factory, plugins, finder, keys, identities),
            }
        }

        pub async fn query_all(&self) -> Result<Vec<PersistedEntity>> {
            self.domain.query_all()
        }

        #[cfg(test)]
        pub async fn snapshot(&self) -> Result<serde_json::Value> {
            let json: Vec<serde_json::Value> = self
                .query_all()
                .await?
                .into_iter()
                .map(|p| p.to_json_value())
                .collect::<Result<_>>()?;

            Ok(serde_json::Value::Array(json))
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
                        if let Some(reply) = session.evaluate_and_perform(USERNAME, text)? {
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
        let storage_factory = sqlite::Factory::new(sqlite::MEMORY_SPECIAL)?;
        let mut registered_plugins = RegisteredPlugins::default();
        registered_plugins.register(MovingPluginFactory::default());
        registered_plugins.register(LookingPluginFactory::default());
        registered_plugins.register(CarryingPluginFactory::default());
        registered_plugins.register(BuildingPluginFactory::default());
        registered_plugins.register(DynamicPluginFactory::default());
        registered_plugins.register(RunePluginFactory::default());
        registered_plugins.register(WasmPluginFactory::default());
        registered_plugins.register(RpcPluginFactory::start().await?);
        let finder = Arc::new(DefaultFinder::default());
        let keys = Arc::new(DeterministicKeys::new());
        let identities = Arc::new(DeterministicKeys::new());
        Ok(AsyncFriendlyDomain::new(
            storage_factory,
            Arc::new(registered_plugins),
            finder,
            keys,
            identities,
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
    struct HoldingKeyInVessel {}

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

    #[tokio::test]
    async fn it_evaluates_a_simple_look() -> Result<()> {
        let domain = test_domain().await?;
        domain.evaluate::<HoldingKeyInVessel>(&["look"]).await?;
        insta::assert_json_snapshot!(domain.snapshot().await?);
        domain.stop().await?;

        Ok(())
    }

    #[tokio::test]
    async fn it_evaluates_two_simple_looks_same_session() -> Result<()> {
        let domain = test_domain().await?;
        domain
            .evaluate::<HoldingKeyInVessel>(&["look", "look"])
            .await?;
        insta::assert_json_snapshot!(domain.snapshot().await?);
        domain.stop().await?;

        Ok(())
    }

    #[tokio::test]
    async fn it_evaluates_two_simple_looks_separate_session() -> Result<()> {
        let domain = test_domain().await?;
        domain.evaluate::<HoldingKeyInVessel>(&["look"]).await?;
        domain.evaluate::<Noop>(&["look"]).await?;
        insta::assert_json_snapshot!(domain.snapshot().await?);
        domain.stop().await?;

        Ok(())
    }

    #[tokio::test]
    async fn it_can_drop_held_container() -> Result<()> {
        let domain = test_domain().await?;
        domain.evaluate::<HoldingKeyInVessel>(&["look"]).await?;
        domain.evaluate::<Noop>(&["drop vessel"]).await?;
        insta::assert_json_snapshot!(domain.snapshot().await?);
        domain.stop().await?;

        Ok(())
    }

    #[tokio::test]
    async fn it_can_rehold_dropped_container() -> Result<()> {
        let domain = test_domain().await?;
        domain.evaluate::<HoldingKeyInVessel>(&["look"]).await?;
        domain.evaluate::<Noop>(&["drop vessel"]).await?;
        domain.evaluate::<Noop>(&["hold vessel"]).await?;
        insta::assert_json_snapshot!(domain.snapshot().await?);
        domain.stop().await?;

        Ok(())
    }

    #[tokio::test]
    async fn it_can_go_east() -> Result<()> {
        let domain = test_domain().await?;
        domain.evaluate::<HoldingKeyInVessel>(&["look"]).await?;
        domain.evaluate::<Noop>(&["go east"]).await?;
        domain.evaluate::<Noop>(&["look"]).await?;
        insta::assert_json_snapshot!(domain.snapshot().await?);
        domain.stop().await?;

        Ok(())
    }
}

#[cfg(test)]
#[ctor::ctor]
fn initialize_tests() {
    plugins_core::log_test();
}
