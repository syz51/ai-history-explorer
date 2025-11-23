use std::hint::black_box;

use ai_history_explorer::filters::apply::apply_filters;
use ai_history_explorer::filters::parser::parse_filter;
use ai_history_explorer::models::{EntryType, SearchEntry};
use chrono::Utc;
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

/// Generate synthetic SearchEntry data
fn generate_search_entries(num_entries: usize) -> Vec<SearchEntry> {
    (0..num_entries)
        .map(|i| SearchEntry {
            entry_type: if i % 2 == 0 { EntryType::UserPrompt } else { EntryType::AgentMessage },
            display_text: format!("Test entry {}", i),
            timestamp: Utc::now(),
            project_path: if i % 3 == 0 {
                Some(format!("/Users/test/project-{}", i % 5).into())
            } else {
                None
            },
            session_id: format!("session-{}", i),
        })
        .collect()
}

fn bench_filter_application(c: &mut Criterion) {
    let mut group = c.benchmark_group("filter_application");

    // Benchmark type filter (simple comparison)
    for size in [1_000, 10_000, 50_000].iter() {
        let entries = generate_search_entries(*size);
        let filter_expr = parse_filter("type:user").unwrap();

        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::new("type_filter", size), size, |b, _| {
            b.iter(|| apply_filters(black_box(entries.clone()), black_box(&filter_expr)).unwrap());
        });
    }

    // Benchmark project filter (string matching)
    for size in [1_000, 10_000, 50_000].iter() {
        let entries = generate_search_entries(*size);
        let filter_expr = parse_filter("project:project-1").unwrap();

        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::new("project_filter", size), size, |b, _| {
            b.iter(|| apply_filters(black_box(entries.clone()), black_box(&filter_expr)).unwrap());
        });
    }

    // Benchmark complex filter (type AND project)
    for size in [1_000, 10_000, 50_000].iter() {
        let entries = generate_search_entries(*size);
        let filter_expr = parse_filter("project:project-1 type:user").unwrap();

        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::new("complex_filter", size), size, |b, _| {
            b.iter(|| apply_filters(black_box(entries.clone()), black_box(&filter_expr)).unwrap());
        });
    }

    group.finish();
}

criterion_group!(benches, bench_filter_application);
criterion_main!(benches);
