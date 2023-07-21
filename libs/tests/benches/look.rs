use anyhow::Result;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use pprof::criterion::{Output, PProfProfiler};
use std::sync::Arc;

use engine::sequences::DeterministicKeys;
use engine::Domain;
use kernel::RegisteredPlugins;
use plugins_core::building::BuildingPluginFactory;
use plugins_core::carrying::CarryingPluginFactory;
use plugins_core::looking::LookingPluginFactory;
use plugins_core::moving::MovingPluginFactory;
use plugins_core::DefaultFinder;
use sqlite::Factory;
use tests::{evaluate_fixture, HoldingKeyInVessel, Noop};

pub fn make_domain() -> Result<Domain> {
    let storage_factory = Factory::new("world.sqlite3")?;
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

const USERNAME: &str = "burrow";

pub fn evaluate_text_in_new_domain(
    username: &str,
    times: usize,
    text: &'static [&'static str],
) -> Result<()> {
    let domain = make_domain()?;

    assert!(times > 0);

    evaluate_fixture::<HoldingKeyInVessel, _>(&domain, username, text)?;

    for _ in [0..times - 1] {
        evaluate_fixture::<Noop, _>(&domain, username, text)?;
    }

    Ok(())
}

pub fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("look ", |b| {
        b.iter(|| black_box(evaluate_text_in_new_domain(USERNAME, 1, &["look"])))
    });

    c.bench_function("look 10 times", |b| {
        b.iter(|| black_box(evaluate_text_in_new_domain(USERNAME, 10, &["look"])))
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().with_profiler(PProfProfiler::new(1000, Output::Flamegraph(None)));
    targets = criterion_benchmark
}

criterion_main!(benches);
