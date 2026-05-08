/// Benchmark: rslm-store — chunk ingestion and search throughput.
///
/// Uses an in-memory SQLite store (tempfile). No embedder — hybrid search
/// falls back to BM25 when no query vector is available.
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use rslm_store::{bm25::Bm25Index, ChunkStore, ChunkStrategy};
use tempfile::NamedTempFile;

const MINI_REDIS_SRC: &str = include_str!("../../../docs/demo/context.txt");
const DOC_ID: &str = "mini-redis";

fn make_store() -> (ChunkStore, NamedTempFile) {
    let f = NamedTempFile::new().unwrap();
    let store = ChunkStore::open(f.path().to_str().unwrap()).unwrap();
    (store, f)
}

fn rt() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

fn bench_ingest(c: &mut Criterion) {
    let mut group = c.benchmark_group("store/ingest");
    for (label, strategy) in [
        ("paragraph", ChunkStrategy::Paragraph),
        ("line_20", ChunkStrategy::Line { count: 20 }),
        (
            "fixed_512",
            ChunkStrategy::Fixed {
                size: 512,
                overlap: 64,
            },
        ),
    ] {
        group.bench_with_input(BenchmarkId::from_parameter(label), &strategy, |b, strat| {
            b.iter(|| {
                let (store, _f) = make_store();
                rt().block_on(store.ingest(DOC_ID, MINI_REDIS_SRC, strat, None))
                    .unwrap();
            })
        });
    }
    group.finish();
}

fn bench_bm25_build(c: &mut Criterion) {
    let chunks: Vec<(usize, String)> = MINI_REDIS_SRC
        .lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
        .map(|(i, l)| (i, l.to_string()))
        .collect();
    let indexed: Vec<(usize, &str)> = chunks.iter().map(|(i, t)| (*i, t.as_str())).collect();

    c.bench_function("store/bm25_build", |b| {
        b.iter(|| Bm25Index::build(black_box(&indexed)))
    });
}

fn bench_bm25_search(c: &mut Criterion) {
    let chunks: Vec<(usize, String)> = MINI_REDIS_SRC
        .lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
        .map(|(i, l)| (i, l.to_string()))
        .collect();
    let indexed: Vec<(usize, &str)> = chunks.iter().map(|(i, t)| (*i, t.as_str())).collect();
    let index = Bm25Index::build(&indexed);

    let mut group = c.benchmark_group("store/bm25_search");
    for (label, query, k) in [
        ("top1", "Semaphore connections", 1usize),
        ("top5", "Semaphore connections", 5),
        ("top10_broad", "fn async pub", 10),
    ] {
        group.bench_with_input(
            BenchmarkId::from_parameter(label),
            &(query, k),
            |b, &(q, k)| b.iter(|| index.search(black_box(q), black_box(k))),
        );
    }
    group.finish();
}

fn bench_store_get_chunks(c: &mut Criterion) {
    let (store, _f) = make_store();
    rt().block_on(store.ingest(DOC_ID, MINI_REDIS_SRC, &ChunkStrategy::Paragraph, None))
        .unwrap();

    c.bench_function("store/get_chunks", |b| {
        b.iter(|| store.get_chunks(black_box(DOC_ID)).unwrap())
    });
}

criterion_group!(
    benches,
    bench_ingest,
    bench_bm25_build,
    bench_bm25_search,
    bench_store_get_chunks,
);
criterion_main!(benches);
