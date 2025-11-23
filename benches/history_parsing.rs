use std::hint::black_box;
use std::io::Write;

use ai_history_explorer::parsers::history::parse_history_file;
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use tempfile::NamedTempFile;

/// Generate a synthetic history.jsonl file with N entries
fn generate_history_file(num_entries: usize) -> NamedTempFile {
    let mut file = NamedTempFile::new().unwrap();

    for i in 0..num_entries {
        let entry = format!(
            r#"{{"display":"Test prompt {}","timestamp":"2024-01-{:02}T12:00:00Z","sessionId":"550e8400-e29b-41d4-a716-{:012x}","messages":[{{"type":"user","text":"Test prompt {}"}}]}}"#,
            i,
            (i % 28) + 1,
            i,
            i
        );
        writeln!(file, "{}", entry).unwrap();
    }

    file.flush().unwrap();
    file
}

fn bench_parse_history(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_history_file");

    for size in [100, 1_000, 10_000, 50_000].iter() {
        let file = generate_history_file(*size);

        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| parse_history_file(black_box(file.path())).unwrap());
        });
    }

    group.finish();
}

criterion_group!(benches, bench_parse_history);
criterion_main!(benches);
