//! Performance benchmarks for BM25 text retrieval.
//!
//! Measures throughput and latency of BM25 search across different corpus sizes.
use agent_core::memory::episodic::EpisodicMemory;
use criterion::{black_box, criterion_group, criterion_main, Criterion};

/// Generate synthetic episodes for benchmark scaling.
fn generate_episodes(count: usize) -> EpisodicMemory {
    let mut mem = EpisodicMemory::new();
    for i in 0..count {
        match i % 3 {
            0 => {
                mem.record_user_request(
                    &format!("create entity_{} with position and sprite", i % 20),
                    Some(serde_json::json!({"entity": format!("entity_{}", i % 20)})),
                );
            }
            1 => {
                mem.record_tool_call(
                    &format!("tool_{}", i % 5),
                    serde_json::json!({"params": format!("operation {}", i)}),
                    Some(serde_json::json!({"result": format!("success {}", i)})),
                    i % 2 == 0,
                );
            }
            _ => {
                mem.record_error(
                    &format!("failed to process entity_{} due to config mismatch", i % 20),
                    None,
                );
            }
        }
    }
    mem
}

fn bench_bm25_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("bm25_search");
    group.sample_size(50);

    for size in [500, 2000, 5000].iter() {
        group.bench_with_input(format!("search_{}_docs", size), size, |b, &size| {
            b.iter_batched(
                || generate_episodes(size),
                |mut mem| {
                    let results = mem.search(black_box("create entity with sprite"), black_box(10));
                    black_box(results);
                },
                criterion::BatchSize::LargeInput,
            );
        });
    }

    group.finish();
}

fn bench_bm25_record(c: &mut Criterion) {
    let mut group = c.benchmark_group("bm25_record");

    group.bench_function("record_100", |b| {
        b.iter_batched(
            EpisodicMemory::new,
            |mut mem| {
                for i in 0..100 {
                    mem.record_tool_call(
                        black_box(&format!("tool_{}", i % 5)),
                        black_box(serde_json::json!({"p": i})),
                        black_box(Some(serde_json::json!({"r": i}))),
                        black_box(i % 2 == 0),
                    );
                }
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.bench_function("record_1000", |b| {
        b.iter_batched(
            EpisodicMemory::new,
            |mut mem| {
                for i in 0..1000 {
                    mem.record_user_request(black_box(&format!("request {}", i)), black_box(None));
                }
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

criterion_group!(benches, bench_bm25_search, bench_bm25_record);
criterion_main!(benches);
