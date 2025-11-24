use std::hint::black_box;

use ai_history_explorer::models::{EntryType, SearchEntry};
use chrono::Utc;
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

/// Generate synthetic SearchEntry data
fn generate_search_entries(num_entries: usize) -> Vec<SearchEntry> {
    (0..num_entries)
        .map(|i| SearchEntry {
            entry_type: if i % 2 == 0 { EntryType::UserPrompt } else { EntryType::AgentMessage },
            display_text: format!("Test entry {} with some content for fuzzy matching", i),
            timestamp: Utc::now(),
            project_path: if i % 3 == 0 {
                Some(format!("/Users/test/project-{}", i % 10).into())
            } else {
                None
            },
            session_id: format!("session-{}", i),
        })
        .collect()
}

fn bench_build_index(c: &mut Criterion) {
    let mut group = c.benchmark_group("build_index");

    for size in [1_000, 10_000, 50_000, 100_000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            // Pre-generate entries outside the benchmark
            let entries = generate_search_entries(size);

            b.iter(|| {
                // Benchmark just the index building/sorting logic
                let mut cloned_entries = black_box(entries.clone());
                cloned_entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
                cloned_entries
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_build_index);
criterion_main!(benches);
