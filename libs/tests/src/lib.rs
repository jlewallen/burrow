#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use anyhow::Result;
    use engine::{storage, DevNullNotifier, Domain};
    use kernel::RegisteredPlugins;
    use plugins_core::{
        building::BuildingPluginFactory, carrying::CarryingPluginFactory,
        /* dynamic::DynamicPluginFactory, */ looking::LookingPluginFactory,
        moving::MovingPluginFactory, BuildSurroundings, DefaultFinder, QuickThing,
    };
    use plugins_rpc::RpcPluginFactory;
    use plugins_rune::RunePluginFactory;
    use plugins_wasm::WasmPluginFactory;
    use tokio::{runtime::Handle, task::JoinHandle};

    async fn make_domain() -> Result<Domain> {
        let storage_factory = storage::sqlite::Factory::new(":memory:")?;
        let mut registered_plugins = RegisteredPlugins::default();
        registered_plugins.register(MovingPluginFactory::default());
        registered_plugins.register(LookingPluginFactory::default());
        registered_plugins.register(CarryingPluginFactory::default());
        registered_plugins.register(BuildingPluginFactory::default());
        // registered_plugins.register(DynamicPluginFactory::default());
        registered_plugins.register(RunePluginFactory::default());
        registered_plugins.register(WasmPluginFactory::default());
        registered_plugins.register(RpcPluginFactory::start(Handle::current()).await?);
        let finder = Arc::new(DefaultFinder::default());
        Ok(Domain::new(
            storage_factory,
            Arc::new(registered_plugins),
            finder,
            true,
        ))
    }

    async fn evaluate(domain: &engine::Domain, text: &'static [&'static str]) -> Result<()> {
        let handle: JoinHandle<Result<()>> = tokio::task::spawn_blocking({
            let domain = domain.clone();
            move || {
                let session = domain.open_session()?;

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

    #[tokio::test]
    async fn it_evaluates_a_simple_look() -> Result<()> {
        let domain = make_domain().await?;

        evaluate(&domain, &["look"]).await?;

        tokio::task::spawn_blocking(move || domain.stop()).await??;

        Ok(())
    }

    #[tokio::test]
    async fn it_evaluates_two_simple_looks_same_session() -> Result<()> {
        let domain = make_domain().await?;

        evaluate(&domain, &["look", "look"]).await?;

        tokio::task::spawn_blocking(move || domain.stop()).await??;

        Ok(())
    }

    #[tokio::test]
    async fn it_evaluates_two_simple_looks_separate_session() -> Result<()> {
        let domain = make_domain().await?;

        evaluate(&domain, &["look"]).await?;
        evaluate(&domain, &["look"]).await?;

        tokio::task::spawn_blocking(move || domain.stop()).await??;

        Ok(())
    }
}

#[cfg(test)]
#[ctor::ctor]
fn initialize_tests() {
    plugins_core::log_test();
}
