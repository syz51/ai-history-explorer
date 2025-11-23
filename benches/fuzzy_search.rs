use std::hint::black_box;
use std::sync::Arc;

use ai_history_explorer::models::{EntryType, SearchEntry};
use chrono::Utc;
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use nucleo::{Config, Nucleo};

/// Generate synthetic SearchEntry data with varied content
fn generate_search_entries(num_entries: usize) -> Vec<SearchEntry> {
    let words = [
        "implement",
        "feature",
        "refactor",
        "bugfix",
        "optimize",
        "test",
        "documentation",
        "database",
        "frontend",
        "backend",
        "API",
        "authentication",
        "performance",
    ];

    (0..num_entries)
        .map(|i| {
            let word = words[i % words.len()];
            SearchEntry {
                entry_type: EntryType::UserPrompt,
                display_text: format!("{} task {} with additional context for matching", word, i),
                timestamp: Utc::now(),
                project_path: None,
                session_id: format!("session-{}", i),
            }
        })
        .collect()
}

fn bench_fuzzy_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("fuzzy_search");

    // Benchmark different dataset sizes with fixed query
    for size in [1_000, 10_000, 50_000].iter() {
        let entries = generate_search_entries(*size);

        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                let mut nucleo: Nucleo<SearchEntry> =
                    Nucleo::new(Config::DEFAULT, Arc::new(|| {}), None, 1);

                let injector = nucleo.injector();
                for entry in &entries {
                    let display_text = entry.display_text.clone();
                    injector.push(entry.clone(), move |_entry, cols| {
                        cols[0] = display_text.clone().into();
                    });
                }

                nucleo.pattern.reparse(
                    0,
                    black_box("implement feature"),
                    nucleo::pattern::CaseMatching::Smart,
                    nucleo::pattern::Normalization::Smart,
                    false,
                );

                nucleo.tick(10);

                let snapshot = nucleo.snapshot();
                snapshot.matched_item_count()
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_fuzzy_search);
criterion_main!(benches);
