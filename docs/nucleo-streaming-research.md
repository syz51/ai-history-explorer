# Nucleo-Picker Streaming API Research

**Date:** 2025-11-22
**Researcher:** Worker A (TUI Stream)

## Summary

nucleo-picker **supports streaming architecture** via Injector API. Recommend streaming for Phase 2 to enable progressive loading and better memory profile.

## Findings

### Streaming Support: YES

- **Injector API**: `picker.injector()` returns handle for concurrent item addition
- **Push method**: `injector.push(item)` adds items while picker runs
- **Thread-safe**: Injector is `Send + Sync`, cloneable across threads
- **Lock-free**: Fully concurrent, wait-free streaming (from nucleo core)
- **Progressive updates**: Matcher runs on background threadpool, UI updates live

### Architecture Pattern

```rust
// Create picker with custom renderer
let mut picker = Picker::new(SearchEntryRenderer);
let injector = picker.injector();

// Spawn thread to stream entries
thread::spawn(move || {
    for entry in build_index()? {
        injector.push(entry);
    }
});

// Pick() blocks until user selects or quits
match picker.pick()? {
    Some(entry) => // handle selection
    None => // user quit
}
```

### Custom Type Integration

- **Render trait**: Must implement for SearchEntry
  - `type Str<'a>`: Associated string type
  - `fn render<'a>(&self, value: &'a SearchEntry) -> Self::Str<'a>`
- **Requirements**: `T: Send + Sync + 'static`
- SearchEntry already satisfies these (uses String, DateTime, PathBuf)

### Batch vs Streaming Comparison

**Current (Batch)**:
- Build entire Vec<SearchEntry> upfront
- Sort by timestamp (newest first)
- Pass all at once to TUI

**Streaming**:
- Stream entries as parsed (history.jsonl → agent files)
- Nucleo handles sorting/matching concurrently
- UI responsive immediately (shows partial results)

**Trade-offs**:

| Aspect | Batch (Vec) | Streaming (Injector) |
|--------|-------------|----------------------|
| Memory | High (all in RAM) | Lower (nucleo buffers) |
| Startup | Slower (wait for all) | Faster (progressive) |
| Complexity | Simpler | More complex (threading) |
| Error handling | Easier (fail early) | Harder (partial results) |

## Decision: Use Streaming

**Rationale**:
1. **UX**: Progressive loading = faster perceived startup
2. **Scalability**: Better for 100K+ entries (user mentioned in plan)
3. **Alignment**: Matches nucleo's design (lock-free streaming)
4. **Phase 2 goal**: Build robust foundation for future features

**Implementation changes needed**:
- Refactor `build_index()` to accept `Injector` callback
- OR: Keep `build_index()` as-is, stream from Vec in TUI layer
- Handle errors mid-stream (log warnings, continue)
- Ensure timestamp ordering (nucleo may not preserve insertion order)

## Ratatui Integration

**Status**: No direct ratatui integration in nucleo-picker

**Approach**: nucleo-picker owns the TUI loop
- Uses built-in rendering (not ratatui-based)
- Provides `pick()` blocking API (returns selection)
- Extended event system for custom integration (not explored yet)

**Concern for Phase 2 plan**:
- Plan assumes ratatui for split-pane layout + preview
- nucleo-picker may not support custom layouts (just results list)
- **Need to verify**: Can we use ratatui alongside nucleo-picker?

## Open Questions

1. **Layout flexibility**: Can nucleo-picker render in ratatui frame, or does it own terminal?
2. **Preview pane**: If nucleo owns terminal, how to show preview alongside results?
3. **Status bar**: Can we customize status bar with ratatui, or use nucleo's built-in?
4. **Timestamp ordering**: Does nucleo preserve insertion order, or re-sort by match score?

## Recommendation

**Option A (nucleo-picker owns TUI)**:
- Use nucleo-picker's built-in TUI
- Limited customization (no split-pane preview)
- Fast implementation, proven performance
- **Downside**: Doesn't match Phase 2 spec (preview pane required)

**Option B (Custom ratatui + nucleo lib)**:
- Use underlying `nucleo` crate (not nucleo-picker)
- Build custom ratatui layout with split panes
- Integrate nucleo matching manually
- **Downside**: More complex, but full control

**Option C (Hybrid - verify feasibility)**:
- Use nucleo-picker for fuzzy matching logic
- Somehow integrate with ratatui for layout
- **Needs research**: API compatibility unknown

## FINAL DECISION: Option B (Custom ratatui + nucleo lib)

**Confirmed approach**: Use `nucleo` crate (NOT nucleo-picker) with custom ratatui TUI

**Rationale**:
- nucleo provides non-blocking background matching via `Nucleo` struct
- `tick()` returns `Snapshot` with match results (never blocks UI thread)
- `Injector` enables streaming items while TUI runs
- Full control over layout (split-pane + preview pane)
- Matches Phase 2 spec requirements exactly

**Implementation**:
```rust
// 1. Create Nucleo matcher
let nucleo = Nucleo::new(Config::DEFAULT, Arc::new(|| {}), None, num_threads);
let injector = nucleo.injector();

// 2. Stream entries in background
thread::spawn(move || {
    for entry in build_index()? {
        injector.push(entry, |e, cols| cols[0] = e.display_text.into());
    }
});

// 3. Custom ratatui event loop
loop {
    // Get latest match results (non-blocking)
    let snapshot = nucleo.snapshot();
    let matches = snapshot.matched_items();

    // Render with ratatui (split panes, preview, etc.)
    terminal.draw(|f| render_ui(f, matches, selected_idx))?;

    // Handle keyboard events
    if let Event::Key(key) = event::read()? {
        // Update search query, selection, etc.
        nucleo.pattern.reparse(...);
    }
}
```

**Dependencies**:
- `nucleo = "0.6"` (NOT nucleo-picker)
- `ratatui = "0.29"`
- `crossterm = "0.28"`

**Next**: Implement TUI with this architecture

## References

- nucleo-picker: https://crates.io/crates/nucleo-picker
- nucleo core: https://docs.rs/nucleo
- GitHub: https://github.com/autobib/nucleo-picker
