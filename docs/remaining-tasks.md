# Remaining Tasks

**Date**: 2025-11-23
**Status**: Post Phase 1-2 Review - LOW PRIORITY TASKS COMPLETED

All low-priority tasks from Phase 1-2 review have been completed:

---

## ✅ Completed Tasks (2025-11-23)

### 1. ✅ Add Module-Level Documentation to TUI Modules

**Status**: COMPLETED
**Files documented**:

- ✅ `src/tui/app.rs` - Added comprehensive module docs with architecture overview
- ✅ `src/tui/rendering.rs` - Added module docs with layout diagram
- ✅ `src/filters/parser.rs` - Added extensive examples and syntax documentation

All modules now have detailed `//!` comments explaining purpose, architecture, and usage.

---

### 2. ✅ Add Criterion Benchmarks for Performance Baselines

**Status**: COMPLETED
**Implemented**:

- ✅ Added `criterion` to dev-dependencies with HTML reports
- ✅ Created `benches/` directory
- ✅ Implemented 4 benchmark suites:
  - `history_parsing.rs` - Parse 100/1K/10K/50K entries
  - `index_building.rs` - Index 1K/10K/50K/100K entries
  - `fuzzy_search.rs` - Fuzzy search on 1K/10K/50K entries
  - `filter_application.rs` - Type/project/complex filters on varying sizes
- ✅ Created `docs/benchmarks.md` with baseline targets and optimization guidelines
- ✅ Updated README with benchmark instructions

**Next Steps**: Run initial benchmarks to establish baselines, revisit when performance issues reported.

---

## Tasks Explicitly Deferred (Per Review)

### Windows Support

- Deferred to Phase 3+
- Tracked in separate roadmap

### Memory Optimization for Large Datasets

- **Trigger**: Users report >50K entry slowness
- **Action**: Use `Arc<SearchEntry>` to reduce cloning
- **Current**: Acceptable for <50K entries (3× memory overhead)

### Parallel Parsing Benchmarking

- **Status**: Implemented with rayon
- **Action needed**: Measure actual speedup
- **Defer**: Until baseline benchmarks established

### Streaming Index Architecture

- **Not recommended**: Adds complexity, minimal benefit
- **Defer**: Until users report >5s startup with >100K entries

---

## Completed (Reference)

✅ ANSI escape code sanitization
✅ Directory symlink TOCTOU fix
✅ Parallel parsing with rayon
✅ Dirty state tracking for TUI
✅ Manual testing checklist
✅ README platform support documentation
