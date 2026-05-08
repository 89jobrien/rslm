/// Benchmark: full RLM loop iterations via HarnessProvider.
///
/// Measures end-to-end loop cost: provider dispatch, script execution,
/// state checks, and message accumulation — no real LLM involved.
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use rslm_harness::Harness;

fn rt() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
}

fn bench_single_step(c: &mut Criterion) {
    let runtime = rt();
    c.bench_function("rlm_loop/single_step", |b| {
        b.to_async(&runtime).iter(|| async {
            Harness::run("q", "ctx", vec![r#"final_answer("done")"#])
                .await
                .unwrap()
        })
    });
}

fn bench_multi_step(c: &mut Criterion) {
    let runtime = rt();
    let mut group = c.benchmark_group("rlm_loop/multi_step");
    for steps in [2usize, 5, 10] {
        group.bench_with_input(BenchmarkId::from_parameter(steps), &steps, |b, &n| {
            b.to_async(&runtime).iter(|| async move {
                let mut responses: Vec<&str> = vec!["ctx_len()"; n - 1];
                responses.push(r#"final_answer("done")"#);
                Harness::run("q", "ctx", responses).await.unwrap()
            })
        });
    }
    group.finish();
}

fn bench_error_recovery(c: &mut Criterion) {
    let runtime = rt();
    c.bench_function("rlm_loop/error_recovery", |b| {
        b.to_async(&runtime).iter(|| async {
            Harness::run(
                "q",
                "ctx",
                vec!["bad rhai @@@@", r#"final_answer("recovered")"#],
            )
            .await
            .unwrap()
        })
    });
}

fn bench_grep_then_answer(c: &mut Criterion) {
    let runtime = rt();
    let ctx = include_str!("../../../docs/demo/context.txt");
    c.bench_function("rlm_loop/grep_then_answer", |b| {
        b.to_async(&runtime).iter(|| async {
            Harness::run(
                "find Semaphore",
                ctx,
                vec![
                    r#"let r = ctx_grep("Semaphore"); print_cell(r)"#,
                    r#"final_answer("done")"#,
                ],
            )
            .await
            .unwrap()
        })
    });
}

criterion_group!(
    benches,
    bench_single_step,
    bench_multi_step,
    bench_error_recovery,
    bench_grep_then_answer,
);
criterion_main!(benches);
