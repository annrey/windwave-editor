//! Performance benchmarks for multi-tier memory system.
//!
//! Measures read/write latency across working and episodic memory layers.
use agent_core::memory::{MemoryConfig, MemoryQuery, MemorySystem};
use criterion::{black_box, criterion_group, criterion_main, Criterion};

/// Create a MemorySystem pre-populated with episodic entries for retrieval benchmarks.
fn create_populated_memory(count: usize) -> MemorySystem {
    let config = MemoryConfig {
        enable_hybrid_retrieval: true,
        ..Default::default()
    };
    let mut system = MemorySystem::with_config(config);
    for i in 0..count {
        match i % 3 {
            0 => {
                system.record_user_request(
                    &format!("request {} about entity {}", i, i % 10),
                    Some(serde_json::json!({"entity": format!("entity_{}", i % 10)})),
                );
            }
            1 => {
                system.record_tool_call(
                    &format!("tool_{}", i % 5),
                    serde_json::json!({"params": format!("op {}", i)}),
                    Some(serde_json::json!({"result": format!("ok {}", i)})),
                    i % 2 == 0,
                );
            }
            _ => {
                system.record_error(&format!("error {} during operation", i), None);
            }
        }
    }
    system
}

fn bench_memory_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_write");
    group.sample_size(100);

    group.bench_function("record_user_request", |b| {
        b.iter_batched(
            MemorySystem::new,
            |mut sys| {
                sys.record_user_request(
                    black_box("create a new player entity with position, scale, and rotation"),
                    black_box(Some(serde_json::json!({
                        "entity": "Player",
                        "components": ["Transform", "Sprite", "Health"]
                    }))),
                );
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.bench_function("record_tool_call", |b| {
        b.iter_batched(
            MemorySystem::new,
            |mut sys| {
                sys.record_tool_call(
                    black_box("edit_file"),
                    black_box(
                        serde_json::json!({"path": "src/main.rs", "old": "foo", "new": "bar"}),
                    ),
                    black_box(Some(serde_json::json!({"status": "success"}))),
                    black_box(true),
                );
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

fn bench_memory_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_read");
    group.sample_size(50);

    for size in [100, 1000, 5000].iter() {
        group.bench_with_input(format!("retrieve_{}_exact", size), size, |b, &size| {
            b.iter_batched(
                || create_populated_memory(size),
                |mut sys| {
                    let query = MemoryQuery::new(black_box("entity request tool"));
                    let results = sys.retrieve(&query);
                    black_box(results);
                },
                criterion::BatchSize::LargeInput,
            );
        });

        group.bench_with_input(format!("retrieve_{}_fuzzy", size), size, |b, &size| {
            b.iter_batched(
                || create_populated_memory(size),
                |mut sys| {
                    let query = MemoryQuery::new(black_box("player component health stats"));
                    let results = sys.retrieve(&query);
                    black_box(results);
                },
                criterion::BatchSize::LargeInput,
            );
        });
    }

    group.finish();
}

fn bench_memory_bulk(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_bulk");
    group.sample_size(50);

    for count in [50, 200, 500].iter() {
        group.bench_with_input(format!("bulk_record_{}", count), count, |b, &count| {
            b.iter_batched(
                MemorySystem::new,
                |mut sys| {
                    for i in 0..count {
                        sys.record_user_request(
                            black_box(&format!("bulk request {} test", i)),
                            black_box(None),
                        );
                    }
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_memory_write,
    bench_memory_read,
    bench_memory_bulk
);
criterion_main!(benches);
