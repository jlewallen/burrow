use criterion::{black_box, criterion_group, criterion_main, Criterion};
use pprof::criterion::{Output, PProfProfiler};

use tests::{evaluate_text_in_new_domain, HoldingKeyInVessel, USERNAME};

pub fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("look once", |b| {
        b.iter(|| {
            black_box(evaluate_text_in_new_domain::<HoldingKeyInVessel>(
                USERNAME,
                1,
                &["look"],
            ))
        })
    });

    c.bench_function("look 10 times", |b| {
        b.iter(|| {
            black_box(evaluate_text_in_new_domain::<HoldingKeyInVessel>(
                USERNAME,
                10,
                &["look"],
            ))
        })
    });

    c.bench_function("drop", |b| {
        b.iter(|| {
            black_box(evaluate_text_in_new_domain::<HoldingKeyInVessel>(
                USERNAME,
                1,
                &["drop"],
            ))
        })
    });

    c.bench_function("drop hold", |b| {
        b.iter(|| {
            black_box(evaluate_text_in_new_domain::<HoldingKeyInVessel>(
                USERNAME,
                10,
                &["drop vessel", "hold vessel"],
            ))
        })
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().with_profiler(PProfProfiler::new(1000, Output::Flamegraph(None)));
    targets = criterion_benchmark
}

criterion_main!(benches);
