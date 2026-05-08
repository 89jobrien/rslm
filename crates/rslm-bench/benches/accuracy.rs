/// Benchmark: deterministic accuracy over the golden query set.
///
/// Each case runs through the full RLM loop with its ideal harness_responses,
/// then scores the answer against expected_patterns.
/// Measures: loop throughput AND correctness (asserts pass).
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use rslm_bench::accuracy::AccuracyCase;
use rslm_harness::Harness;

const GOLDEN_JSON: &str = include_str!("../golden/queries.json");
const CTX: &str = include_str!("../../../docs/demo/context.txt");

fn load_cases() -> Vec<AccuracyCase> {
    serde_json::from_str(GOLDEN_JSON).expect("parse golden/queries.json")
}

fn rt() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
}

fn bench_accuracy_cases(c: &mut Criterion) {
    let cases = load_cases();
    let runtime = rt();
    let mut group = c.benchmark_group("accuracy");

    for case in cases.iter().filter(|c| !c.harness_responses.is_empty()) {
        group.bench_with_input(BenchmarkId::from_parameter(&case.id), case, |b, case| {
            b.to_async(&runtime).iter(|| async {
                let run = Harness::run(&case.query, CTX, case.harness_responses.clone())
                    .await
                    .unwrap();
                let result = case.score(&run.answer);
                // Assert correctness — a regression here means the engine
                // no longer produces the expected output for a known-good script.
                assert!(
                    result.passed,
                    "accuracy regression on '{}': missing {:?} in answer: {:?}",
                    case.id, result.failed_patterns, run.answer
                );
                run
            })
        });
    }
    group.finish();
}

criterion_group!(benches, bench_accuracy_cases);
criterion_main!(benches);
