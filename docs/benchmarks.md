# Performance Benchmarks

Criterion-based benchmarks for AI History Explorer performance tracking.

## Running Benchmarks

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark
cargo bench --bench history_parsing
cargo bench --bench index_building
cargo bench --bench fuzzy_search
cargo bench --bench filter_application

# View HTML reports
open target/criterion/report/index.html
```

## Benchmark Suites

### 1. History Parsing (`history_parsing`)

**Purpose**: Measure JSONL parsing performance with varying file sizes.

**Test Cases**:

- 100 entries
- 1,000 entries
- 10,000 entries
- 50,000 entries

**Metrics**: Throughput (entries/sec), mean time, standard deviation

**Baseline Targets** (to be established on first run):

- 10K entries: < 100ms
- 50K entries: < 500ms

---

### 2. Index Building (`index_building`)

**Purpose**: Measure index construction and sorting performance.

**Test Cases**:

- 1,000 entries
- 10,000 entries
- 50,000 entries
- 100,000 entries

**Metrics**: Throughput (entries/sec), mean time

**Baseline Targets**:

- 10K entries: < 50ms
- 100K entries: < 500ms

---

### 3. Fuzzy Search (`fuzzy_search`)

**Purpose**: Measure nucleo fuzzy matching performance on various dataset sizes.

**Test Cases**:

- 1,000 entries
- 10,000 entries
- 50,000 entries

**Query**: Fixed pattern "implement feature"

**Metrics**: Throughput (entries searched/sec), latency

**Baseline Targets**:

- 10K entries: < 100ms for full fuzzy match
- 50K entries: < 500ms for full fuzzy match

---

### 4. Filter Application (`filter_application`)

**Purpose**: Measure filter evaluation performance across entry sets.

**Test Cases**:

- **Type filter** (`type:user`): Simple enum comparison
- **Project filter** (`project:project-1`): String matching with path expansion
- **Complex filter** (`project:project-1 type:user`): Combined AND logic

**Dataset Sizes**: 1K, 10K, 50K entries

**Metrics**: Throughput (entries filtered/sec)

**Baseline Targets**:

- Type filter @ 10K: < 10ms
- Project filter @ 10K: < 50ms
- Complex filter @ 10K: < 100ms

---

## Performance Optimization Guidelines

### When to Optimize

Benchmarks establish baselines. Optimize when:

1. **User-reported slowness**: Users complain about lag with their dataset size
2. **Threshold exceeded**: Benchmarks show >2x regression vs baseline
3. **Scale requirements**: Supporting larger datasets (e.g., >100K entries)

### Optimization Strategies

- **Parsing**: Already parallelized with rayon
- **Fuzzy search**: Consider increasing nucleo thread count for large datasets
- **Filters**: Use `Arc<SearchEntry>` to reduce cloning overhead
- **Index**: Evaluate streaming/lazy loading for >100K entries

### Regression Detection

Run benchmarks before releases:

```bash
# Save baseline
cargo bench -- --save-baseline main

# After changes, compare
cargo bench -- --baseline main
```

Criterion will highlight significant performance changes.

---

## Interpreting Results

Criterion produces:

- **Mean time**: Average execution time
- **Std deviation**: Variance in measurements
- **Throughput**: Items processed per second
- **Outliers**: Measurements excluded from analysis

**Good performance**: Low mean, low std deviation, high throughput.

**Regression indicators**: Mean increased >10%, throughput decreased >10%.

---

## Future Benchmarks

Potential additions:

- **Re-injection performance**: Measure nucleo re-injection after filter changes
- **TUI rendering**: Frame rendering latency under load
- **Incremental updates**: Adding new entries to existing index

Add when needed for specific optimization work.
