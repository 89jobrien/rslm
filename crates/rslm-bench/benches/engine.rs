/// Benchmark: Rhai engine script execution throughput.
///
/// Measures how fast the core env functions (ctx_len, ctx_slice, ctx_grep,
/// print_cell, final_answer, rlm_call callback) execute via `run_script`,
/// with no LLM or async overhead.
use std::sync::{Arc, Mutex};

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use rslm_core::env::{build_engine, run_script, CallState};

const SMALL_CTX: &str = "hello world foo bar baz";
const LARGE_CTX: &str = include_str!("../../../docs/demo/context.txt");

fn make_engine(ctx: &str) -> (rhai::Engine, Arc<Mutex<CallState>>) {
    let state = Arc::new(Mutex::new(CallState::default()));
    let engine = build_engine(ctx.to_string(), Arc::clone(&state), |_, _| {
        "stub".to_string()
    });
    (engine, state)
}

fn bench_ctx_len(c: &mut Criterion) {
    let (engine, state) = make_engine(LARGE_CTX);
    c.bench_function("engine/ctx_len", |b| {
        b.iter(|| run_script(&engine, black_box("ctx_len()"), &state).unwrap())
    });
}

fn bench_ctx_slice(c: &mut Criterion) {
    let (engine, state) = make_engine(LARGE_CTX);
    let len = LARGE_CTX.len();
    c.bench_function("engine/ctx_slice_1k", |b| {
        b.iter(|| {
            run_script(
                &engine,
                black_box(&format!("ctx_slice(0, {})", len.min(1024))),
                &state,
            )
            .unwrap()
        })
    });
}

fn bench_ctx_grep(c: &mut Criterion) {
    let (engine, state) = make_engine(LARGE_CTX);
    let mut group = c.benchmark_group("engine/ctx_grep");
    for pattern in ["Semaphore", "shutdown", "fn ", "zzz_no_match"] {
        group.bench_with_input(BenchmarkId::from_parameter(pattern), pattern, |b, pat| {
            let script = format!(r#"ctx_grep("{pat}")"#);
            b.iter(|| run_script(&engine, black_box(&script), &state).unwrap())
        });
    }
    group.finish();
}

fn bench_final_answer(c: &mut Criterion) {
    let (engine, state) = make_engine(SMALL_CTX);
    c.bench_function("engine/final_answer", |b| {
        b.iter(|| {
            // Reset state between iterations so final_answer can be set repeatedly
            state.lock().unwrap().final_answer = None;
            run_script(&engine, black_box(r#"final_answer("ok")"#), &state).unwrap()
        })
    });
}

fn bench_rlm_call_callback(c: &mut Criterion) {
    // Measures the overhead of the rlm_call Rhai→Rust bridge (callback only, no child RLM)
    let (engine, state) = make_engine(SMALL_CTX);
    c.bench_function("engine/rlm_call_stub", |b| {
        b.iter(|| run_script(&engine, black_box(r#"rlm_call("q", "ctx")"#), &state).unwrap())
    });
}

criterion_group!(
    benches,
    bench_ctx_len,
    bench_ctx_slice,
    bench_ctx_grep,
    bench_final_answer,
    bench_rlm_call_callback,
);
criterion_main!(benches);
