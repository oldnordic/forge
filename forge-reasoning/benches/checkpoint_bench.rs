//! TDD Wave 7: Performance Benchmarks
//!
//! Criterion.rs benchmarks for checkpoint operations

use std::sync::Arc;
use std::time::Duration;

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use forge_reasoning::*;

/// Benchmark: Single checkpoint creation
fn bench_checkpoint_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("checkpoint_creation");
    
    group.bench_function("single", |b| {
        let storage = ThreadSafeStorage::in_memory().unwrap();
        let session_id = SessionId::new();
        let manager = ThreadSafeCheckpointManager::new(storage, session_id);
        
        b.iter(|| {
            manager.checkpoint(black_box("Benchmark checkpoint")).unwrap()
        });
    });
    
    group.finish();
}

/// Benchmark: Multiple checkpoint creation (sequential)
fn bench_checkpoint_creation_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("checkpoint_creation_batch");
    
    for size in [10, 100, 1000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let storage = ThreadSafeStorage::in_memory().unwrap();
            let session_id = SessionId::new();
            let manager = ThreadSafeCheckpointManager::new(storage, session_id);
            
            b.iter(|| {
                for i in 0..size {
                    manager.checkpoint(format!("Checkpoint {}", i)).unwrap();
                }
            });
        });
    }
    
    group.finish();
}

/// Benchmark: Query operations
fn bench_query_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("query_operations");
    
    // Setup: Create checkpoints to query
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let session_id = SessionId::new();
    let manager = ThreadSafeCheckpointManager::new(storage.clone(), session_id);
    
    // Pre-populate with 100 checkpoints
    for i in 0..100 {
        manager.checkpoint(format!("Pre-populated {}", i)).unwrap();
    }
    
    group.bench_function("list_all", |b| {
        b.iter(|| {
            let _ = manager.list().unwrap();
        });
    });
    
    group.bench_function("list_by_session", |b| {
        b.iter(|| {
            let _ = manager.list_by_session(&session_id).unwrap();
        });
    });
    
    group.finish();
}

/// Benchmark: Checkpoint with tags
fn bench_checkpoint_with_tags(c: &mut Criterion) {
    let mut group = c.benchmark_group("checkpoint_with_tags");
    
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let session_id = SessionId::new();
    let manager = ThreadSafeCheckpointManager::new(storage, session_id);
    
    let tags = vec![
        "important".to_string(),
        "critical".to_string(),
        "release".to_string(),
    ];
    
    group.bench_function("with_3_tags", |b| {
        b.iter(|| {
            manager.checkpoint_with_tags(
                black_box("Tagged checkpoint"),
                tags.clone()
            ).unwrap()
        });
    });
    
    group.finish();
}

/// Benchmark: Restore operation
fn bench_restore(c: &mut Criterion) {
    let mut group = c.benchmark_group("restore");
    
    // Setup: Create a checkpoint to restore
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let session_id = SessionId::new();
    let manager = ThreadSafeCheckpointManager::new(storage, session_id);
    let checkpoint_id = manager.checkpoint("Restore benchmark").unwrap();
    let checkpoint = manager.get(&checkpoint_id).unwrap().unwrap();
    
    group.bench_function("single", |b| {
        b.iter(|| {
            let _ = manager.restore(&checkpoint).unwrap();
        });
    });
    
    group.finish();
}

/// Benchmark: Compaction
fn bench_compaction(c: &mut Criterion) {
    let mut group = c.benchmark_group("compaction");
    
    for checkpoint_count in [100, 500, 1000].iter() {
        group.throughput(Throughput::Elements(*checkpoint_count as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(checkpoint_count),
            checkpoint_count,
            |b, &count| {
                b.iter_with_setup(
                    || {
                        // Setup: Create many checkpoints
                        let storage = ThreadSafeStorage::in_memory().unwrap();
                        let session_id = SessionId::new();
                        let manager = ThreadSafeCheckpointManager::new(storage, session_id);
                        for i in 0..count {
                            manager.checkpoint(format!("CP {}", i)).unwrap();
                        }
                        manager
                    },
                    |manager| {
                        // Benchmark: Compact to 10 checkpoints
                        manager.compact(10).unwrap();
                    }
                );
            }
        );
    }
    
    group.finish();
}

/// Benchmark: Concurrent checkpoint creation
fn bench_concurrent_creation(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("concurrent_creation");
    
    for num_threads in [2, 4, 8].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_threads),
            num_threads,
            |b, &threads| {
                b.to_async(&runtime).iter(|| async {
                    let storage = ThreadSafeStorage::in_memory().unwrap();
                    let session_id = SessionId::new();
                    
                    let handles: Vec<_> = (0..threads)
                        .map(|thread_id| {
                            let storage = storage.clone();
                            let session_id = session_id.clone();
                            tokio::spawn(async move {
                                let manager = ThreadSafeCheckpointManager::new(storage, session_id);
                                for i in 0..10 {
                                    manager.checkpoint(format!("T{}-CP{}", thread_id, i)).unwrap();
                                }
                            })
                        })
                        .collect();
                    
                    for handle in handles {
                        handle.await.unwrap();
                    }
                });
            }
        );
    }
    
    group.finish();
}

/// Benchmark: Export operation
fn bench_export(c: &mut Criterion) {
    let mut group = c.benchmark_group("export");
    
    for checkpoint_count in [10, 100, 500].iter() {
        group.throughput(Throughput::Elements(*checkpoint_count as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(checkpoint_count),
            checkpoint_count,
            |b, &count| {
                b.iter_with_setup(
                    || {
                        // Setup
                        let storage = ThreadSafeStorage::in_memory().unwrap();
                        let session_id = SessionId::new();
                        let manager = ThreadSafeCheckpointManager::new(storage.clone(), session_id);
                        for i in 0..count {
                            manager.checkpoint(format!("Export CP {}", i)).unwrap();
                        }
                        (storage, session_id)
                    },
                    |(storage, session_id)| {
                        let exporter = CheckpointExporter::new(storage.clone());
                        let _ = exporter.export_session(&session_id).unwrap();
                    }
                );
            }
        );
    }
    
    group.finish();
}

/// Benchmark: Service operations
fn bench_service_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("service_operations");
    
    group.bench_function("create_session", |b| {
        let storage = ThreadSafeStorage::in_memory().unwrap();
        let service = CheckpointService::new(storage);
        
        b.iter(|| {
            let _ = service.create_session(black_box("benchmark-session")).unwrap();
        });
    });
    
    group.bench_function("checkpoint_through_service", |b| {
        let storage = ThreadSafeStorage::in_memory().unwrap();
        let service = CheckpointService::new(storage);
        let session = service.create_session("bench").unwrap();
        
        b.iter(|| {
            let _ = service.checkpoint(&session, black_box("Service checkpoint")).unwrap();
        });
    });
    
    group.bench_function("metrics", |b| {
        let storage = ThreadSafeStorage::in_memory().unwrap();
        let service = CheckpointService::new(storage);
        let session = service.create_session("bench").unwrap();
        for i in 0..10 {
            service.checkpoint(&session, format!("CP {}", i)).unwrap();
        }
        
        b.iter(|| {
            let _ = service.metrics().unwrap();
        });
    });
    
    group.finish();
}

/// Benchmark: File-based storage vs in-memory
fn bench_storage_backends(c: &mut Criterion) {
    let mut group = c.benchmark_group("storage_backends");
    
    group.bench_function("in_memory", |b| {
        let storage = ThreadSafeStorage::in_memory().unwrap();
        let session_id = SessionId::new();
        let manager = ThreadSafeCheckpointManager::new(storage, session_id);
        
        b.iter(|| {
            manager.checkpoint(black_box("In-memory checkpoint")).unwrap()
        });
    });
    
    group.bench_function("file_based", |b| {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let db_path = temp_dir.path().join("bench.db");
        let storage = ThreadSafeStorage::open(&db_path).unwrap();
        let session_id = SessionId::new();
        let manager = ThreadSafeCheckpointManager::new(storage, session_id);
        
        b.iter(|| {
            manager.checkpoint(black_box("File-based checkpoint")).unwrap()
        });
    });
    
    group.finish();
}

criterion_group!(
    benches,
    bench_checkpoint_creation,
    bench_checkpoint_creation_batch,
    bench_checkpoint_with_tags,
    bench_query_operations,
    bench_restore,
    bench_compaction,
    bench_concurrent_creation,
    bench_export,
    bench_service_operations,
    bench_storage_backends
);

criterion_main!(benches);
