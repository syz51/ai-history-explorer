//! TUI application state and event handling.
//!
//! This module implements the main TUI application logic for the AI History Explorer.
//! It manages:
//!
//! - **Fuzzy search**: Integration with `nucleo` for real-time fuzzy matching
//! - **Filter integration**: Parses and applies filters from search query (left of `|`)
//! - **Event loop**: Handles keyboard input and manages application lifecycle
//! - **Status messages**: Transient feedback for clipboard operations and errors
//! - **Dirty state tracking**: Optimized rendering only when state changes
//!
//! # Architecture
//!
//! The `App` struct owns all application state and runs the main event loop via `run()`.
//! Input syntax: `filter_expr | fuzzy_query` where:
//! - Filter portion (left of `|`): Applied when Enter is pressed, reduces entry set
//! - Fuzzy portion (right of `|`): Real-time fuzzy matching via nucleo
//!
//! # Example
//!
//! ```rust,ignore
//! let entries = vec![/* SearchEntry instances */];
//! let mut app = App::new(entries);
//! app.run(&mut terminal)?;
//! ```

use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use nucleo::{Config, Nucleo};
use ratatui::Terminal;
use ratatui::backend::Backend;

use super::events::{Action, poll_event};
use super::rendering::{RenderState, render_ui};
use crate::clipboard::copy_to_clipboard;
use crate::filters::apply::apply_filters;
use crate::filters::ast::FilterExpr;
use crate::filters::parser::parse_filter;
use crate::models::SearchEntry;

/// Duration for success status messages (milliseconds)
const STATUS_SUCCESS_DURATION_MS: u64 = 3000;
/// Duration for error status messages (milliseconds)
const STATUS_ERROR_DURATION_MS: u64 = 5000;

/// Type of status message
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageType {
    Success,
    Error,
}

/// Transient status message with expiry
#[derive(Debug, Clone)]
pub struct StatusMessage {
    pub text: String,
    pub message_type: MessageType,
    pub expires_at: Instant,
}

pub struct App {
    nucleo: Nucleo<SearchEntry>,
    selected_idx: usize,
    search_query: String,
    should_quit: bool,
    // Filter integration fields
    all_entries: Vec<SearchEntry>,
    filtered_entries: Vec<SearchEntry>,
    current_filter: Option<FilterExpr>,
    filter_error: Option<String>,
    last_enter_time: Option<Instant>,
    // Status message (clipboard feedback, etc.)
    status_message: Option<StatusMessage>,
    // Dirty state tracking for efficient rendering
    needs_redraw: bool,
    last_draw_time: Instant,
}

impl App {
    pub fn new(entries: Vec<SearchEntry>) -> Self {
        // Create nucleo matcher with default config
        let nucleo = Nucleo::new(
            Config::DEFAULT,
            Arc::new(|| {}),
            None,
            1, // Single thread for now (can increase for large datasets)
        );

        // Inject all entries
        let injector = nucleo.injector();
        for entry in &entries {
            let display_text = entry.display_text.clone();
            injector.push(entry.clone(), move |_entry, cols| {
                cols[0] = display_text.clone().into();
            });
        }

        // Initialize filter state
        let filtered_entries = entries.clone();

        Self {
            nucleo,
            selected_idx: 0,
            search_query: String::new(),
            should_quit: false,
            all_entries: entries,
            filtered_entries,
            current_filter: None,
            filter_error: None,
            last_enter_time: None,
            status_message: None,
            needs_redraw: true, // Initial draw needed
            last_draw_time: Instant::now(),
        }
    }

    /// Set a transient status message with automatic expiry
    fn set_status(&mut self, text: impl Into<String>, message_type: MessageType, duration_ms: u64) {
        self.status_message = Some(StatusMessage {
            text: text.into(),
            message_type,
            expires_at: Instant::now() + Duration::from_millis(duration_ms),
        });
        self.needs_redraw = true;
    }

    /// Check and clear expired status messages
    fn check_and_clear_expired_status(&mut self) {
        let should_clear = self
            .status_message
            .as_ref()
            .map(|msg| Instant::now() >= msg.expires_at)
            .unwrap_or(false);
        if should_clear {
            self.status_message = None;
        }
    }

    /// Process nucleo updates (tick to process matches)
    fn process_nucleo_updates(&mut self) {
        // Tick nucleo to process matches
        self.nucleo.tick(10);
    }

    pub fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        while !self.should_quit {
            // Clear expired status messages (marks dirty if cleared)
            let had_status = self.status_message.is_some();
            self.check_and_clear_expired_status();
            if had_status && self.status_message.is_none() {
                self.needs_redraw = true;
            }

            // Process nucleo updates
            self.process_nucleo_updates();

            // Get latest match results from nucleo
            let matched_items = self.collect_matched_items();
            let matched_count = matched_items.len();

            // Draw if dirty or if it's been >100ms (for terminal resize handling)
            let now = Instant::now();
            let elapsed = now.duration_since(self.last_draw_time);
            if self.needs_redraw || elapsed >= Duration::from_millis(100) {
                terminal.draw(|f| {
                    let state = RenderState {
                        search_query: &self.search_query,
                        filtered_count: self.filtered_entries.len(),
                        total_count: self.all_entries.len(),
                        filter_error: self.filter_error.as_deref(),
                        status_message: self.status_message.as_ref(),
                    };
                    render_ui(f, &matched_items, self.selected_idx, &state);
                })?;
                self.needs_redraw = false;
                self.last_draw_time = now;
            }

            // Handle events
            let action = poll_event(Duration::from_millis(100))?;
            self.handle_action(action, matched_count);
        }

        Ok(())
    }

    /// Collect matched items from nucleo snapshot (extracted for testing)
    fn collect_matched_items(&self) -> Vec<&SearchEntry> {
        let snapshot = self.nucleo.snapshot();
        snapshot.matched_items(..snapshot.matched_item_count()).map(|item| item.data).collect()
    }

    /// Handle a user action (extracted for testing)
    fn handle_action(&mut self, action: Action, total_items: usize) {
        match action {
            Action::Quit => self.should_quit = true,
            Action::ClearSearch => {
                if self.search_query.is_empty() {
                    self.should_quit = true;
                } else {
                    self.search_query.clear();
                    self.update_nucleo_pattern();
                    self.selected_idx = 0;
                    self.needs_redraw = true;
                }
            }
            Action::MoveUp => self.move_selection(-1, total_items),
            Action::MoveDown => self.move_selection(1, total_items),
            Action::PageUp => self.move_selection(-10, total_items),
            Action::PageDown => self.move_selection(10, total_items),
            Action::UpdateSearch(c) => self.update_search(c),
            Action::DeleteChar => self.delete_char(),
            Action::ApplyFilter => {
                // Debounce: only apply if 150ms has elapsed since last Enter
                let should_apply = if let Some(last_time) = self.last_enter_time {
                    last_time.elapsed() >= Duration::from_millis(150)
                } else {
                    true // First Enter press
                };

                if should_apply {
                    self.apply_filter();
                    self.last_enter_time = Some(Instant::now());
                }
            }
            Action::CopyToClipboard => {
                // Get currently matched items (fuzzy-filtered)
                let matched_items = self.collect_matched_items();

                if matched_items.is_empty() {
                    self.set_status(
                        "✗ No entries to copy",
                        MessageType::Error,
                        STATUS_ERROR_DURATION_MS,
                    );
                } else if self.selected_idx >= matched_items.len() {
                    self.set_status(
                        "✗ Invalid selection",
                        MessageType::Error,
                        STATUS_ERROR_DURATION_MS,
                    );
                } else {
                    // Copy selected entry's display text
                    let entry = matched_items[self.selected_idx];
                    match copy_to_clipboard(&entry.display_text) {
                        Ok(()) => {
                            self.set_status(
                                "✓ Copied to clipboard",
                                MessageType::Success,
                                STATUS_SUCCESS_DURATION_MS,
                            );
                        }
                        Err(e) => {
                            self.set_status(
                                format!("✗ Clipboard error: {}", e),
                                MessageType::Error,
                                STATUS_ERROR_DURATION_MS,
                            );
                        }
                    }
                }
            }
            Action::ToggleFilter => {
                // Stub for Worker C (filters)
            }
            Action::ToggleFocus => {
                // TODO: Implement focus toggle between results and preview
            }
            Action::Refresh => {
                // TODO: Implement index refresh
            }
            Action::None => {}
        }
    }

    fn move_selection(&mut self, delta: isize, total: usize) {
        if total == 0 {
            self.selected_idx = 0;
            return;
        }

        let old_idx = self.selected_idx;
        let new_idx = (self.selected_idx as isize + delta).max(0) as usize;
        self.selected_idx = new_idx.min(total - 1);

        if old_idx != self.selected_idx {
            self.needs_redraw = true;
        }
    }

    fn update_search(&mut self, c: char) {
        // Limit search query to 256 characters to prevent DoS
        if self.search_query.len() < 256 {
            self.search_query.push(c);
            self.update_nucleo_pattern();
            self.selected_idx = 0; // Reset selection on search change
            self.needs_redraw = true;
        }
    }

    fn delete_char(&mut self) {
        if self.search_query.pop().is_some() {
            self.update_nucleo_pattern();
            self.selected_idx = 0;
            self.needs_redraw = true;
        }
    }

    fn update_nucleo_pattern(&mut self) {
        // Extract fuzzy portion (right of |, or full query if no |)
        let fuzzy_query = self.extract_fuzzy_portion();

        self.nucleo.pattern.reparse(
            0,
            &fuzzy_query,
            nucleo::pattern::CaseMatching::Smart,
            nucleo::pattern::Normalization::Smart,
            false,
        );
        // Tick to apply the new pattern
        self.nucleo.tick(10);
    }

    /// Extract filter and fuzzy portions from search_query
    /// Returns (filter_portion, fuzzy_portion)
    fn parse_input(&self) -> (Option<&str>, &str) {
        if let Some(pipe_pos) = self.search_query.find('|') {
            let filter_part = self.search_query[..pipe_pos].trim();
            let fuzzy_part = self.search_query[pipe_pos + 1..].trim();

            let filter = if filter_part.is_empty() { None } else { Some(filter_part) };

            (filter, fuzzy_part)
        } else {
            // No pipe: treat entire input as fuzzy search
            (None, self.search_query.as_str())
        }
    }

    /// Extract only the fuzzy portion for nucleo pattern matching
    fn extract_fuzzy_portion(&self) -> String {
        self.parse_input().1.to_string()
    }

    /// Extract only the filter portion (if present)
    fn extract_filter_portion(&self) -> Option<String> {
        self.parse_input().0.map(|s| s.to_string())
    }

    /// Apply filters from the filter portion of the input
    fn apply_filter(&mut self) {
        // Extract filter portion
        let filter_str = match self.extract_filter_portion() {
            Some(s) => s,
            None => {
                // No filter: reset to all entries
                self.current_filter = None;
                self.filter_error = None;
                self.filtered_entries = self.all_entries.clone();
                self.re_inject_entries();
                self.needs_redraw = true;
                return;
            }
        };

        // Parse filter
        match parse_filter(&filter_str) {
            Ok(filter_expr) => {
                // Apply filter (clone all_entries as apply_filters takes ownership)
                match apply_filters(self.all_entries.clone(), &filter_expr) {
                    Ok(filtered) => {
                        self.filtered_entries = filtered;
                        self.current_filter = Some(filter_expr);
                        self.filter_error = None;
                        self.re_inject_entries();
                        self.needs_redraw = true;
                    }
                    Err(e) => {
                        self.filter_error = Some(format!("Filter error: {}", e));
                        self.needs_redraw = true;
                    }
                }
            }
            Err(e) => {
                self.filter_error =
                    Some(format!("Parse error: {} | Try: project:name type:user | search", e));
                self.needs_redraw = true;
            }
        }
    }

    /// Re-inject filtered entries into nucleo matcher
    fn re_inject_entries(&mut self) {
        // Clear existing entries
        self.nucleo = Nucleo::new(Config::DEFAULT, Arc::new(|| {}), None, 1);

        // Inject filtered entries
        let injector = self.nucleo.injector();
        for entry in &self.filtered_entries {
            let display_text = entry.display_text.clone();
            injector.push(entry.clone(), move |_entry, cols| {
                cols[0] = display_text.clone().into();
            });
        }

        // Re-apply fuzzy pattern
        self.update_nucleo_pattern();

        // Reset selection
        self.selected_idx = 0;
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;
    use crate::tui::events::Action;

    fn create_test_entry() -> SearchEntry {
        SearchEntry {
            entry_type: crate::models::EntryType::UserPrompt,
            display_text: "Test entry".to_string(),
            timestamp: Utc.timestamp_opt(1234567890, 0).unwrap(),
            project_path: None,
            session_id: "test-session".to_string(),
        }
    }

    #[test]
    fn test_app_new_initializes_state() {
        let entries = vec![create_test_entry()];
        let app = App::new(entries);

        assert_eq!(app.selected_idx, 0);
        assert_eq!(app.search_query, "");
        assert!(!app.should_quit);
    }

    #[test]
    fn test_move_selection_down() {
        let entries = vec![create_test_entry(), create_test_entry(), create_test_entry()];
        let mut app = App::new(entries);

        app.move_selection(1, 3);
        assert_eq!(app.selected_idx, 1);

        app.move_selection(1, 3);
        assert_eq!(app.selected_idx, 2);
    }

    #[test]
    fn test_move_selection_up() {
        let entries = vec![create_test_entry(), create_test_entry()];
        let mut app = App::new(entries);
        app.selected_idx = 1;

        app.move_selection(-1, 2);
        assert_eq!(app.selected_idx, 0);
    }

    #[test]
    fn test_move_selection_bounds() {
        let entries = vec![create_test_entry(), create_test_entry()];
        let mut app = App::new(entries);

        // Can't go below 0
        app.move_selection(-10, 2);
        assert_eq!(app.selected_idx, 0);

        // Can't go above len-1
        app.move_selection(10, 2);
        assert_eq!(app.selected_idx, 1);
    }

    #[test]
    fn test_update_search() {
        let entries = vec![create_test_entry()];
        let mut app = App::new(entries);

        app.update_search('a');
        assert_eq!(app.search_query, "a");

        app.update_search('b');
        assert_eq!(app.search_query, "ab");
    }

    #[test]
    fn test_delete_char() {
        let entries = vec![create_test_entry()];
        let mut app = App::new(entries);
        app.search_query = "test".to_string();

        app.delete_char();
        assert_eq!(app.search_query, "tes");

        app.delete_char();
        assert_eq!(app.search_query, "te");
    }

    #[test]
    fn test_delete_char_empty() {
        let entries = vec![create_test_entry()];
        let mut app = App::new(entries);

        app.delete_char();
        assert_eq!(app.search_query, "");
    }

    #[test]
    fn test_handle_action_quit() {
        let entries = vec![create_test_entry()];
        let mut app = App::new(entries);

        assert!(!app.should_quit);
        app.handle_action(Action::Quit, 1);
        assert!(app.should_quit);
    }

    #[test]
    fn test_handle_action_move_up() {
        let entries = vec![create_test_entry(), create_test_entry()];
        let mut app = App::new(entries);
        app.selected_idx = 1;

        app.handle_action(Action::MoveUp, 2);
        assert_eq!(app.selected_idx, 0);
    }

    #[test]
    fn test_handle_action_move_down() {
        let entries = vec![create_test_entry(), create_test_entry()];
        let mut app = App::new(entries);

        app.handle_action(Action::MoveDown, 2);
        assert_eq!(app.selected_idx, 1);
    }

    #[test]
    fn test_handle_action_page_up() {
        let entries = vec![create_test_entry(); 15];
        let mut app = App::new(entries);
        app.selected_idx = 14;

        app.handle_action(Action::PageUp, 15);
        assert_eq!(app.selected_idx, 4);
    }

    #[test]
    fn test_handle_action_page_down() {
        let entries = vec![create_test_entry(); 15];
        let mut app = App::new(entries);

        app.handle_action(Action::PageDown, 15);
        assert_eq!(app.selected_idx, 10);
    }

    #[test]
    fn test_handle_action_update_search() {
        let entries = vec![create_test_entry()];
        let mut app = App::new(entries);

        app.handle_action(Action::UpdateSearch('t'), 1);
        assert_eq!(app.search_query, "t");

        app.handle_action(Action::UpdateSearch('e'), 1);
        assert_eq!(app.search_query, "te");
    }

    #[test]
    fn test_handle_action_delete_char() {
        let entries = vec![create_test_entry()];
        let mut app = App::new(entries);
        app.search_query = "test".to_string();

        app.handle_action(Action::DeleteChar, 1);
        assert_eq!(app.search_query, "tes");
    }

    #[test]
    fn test_handle_action_none() {
        let entries = vec![create_test_entry()];
        let mut app = App::new(entries);
        let initial_state = (app.selected_idx, app.search_query.clone(), app.should_quit);

        app.handle_action(Action::None, 1);

        // State should be unchanged
        assert_eq!(app.selected_idx, initial_state.0);
        assert_eq!(app.search_query, initial_state.1);
        assert_eq!(app.should_quit, initial_state.2);
    }

    #[test]
    fn test_handle_action_copy_to_clipboard_empty_entries() {
        let mut app = App::new(vec![]);
        app.nucleo.tick(10);

        app.handle_action(Action::CopyToClipboard, 0);

        // Should set error status message
        assert!(app.status_message.is_some());
        let msg = app.status_message.as_ref().unwrap();
        assert_eq!(msg.text, "✗ No entries to copy");
        assert_eq!(msg.message_type, MessageType::Error);
    }

    #[test]
    fn test_handle_action_copy_to_clipboard_invalid_selection() {
        let entries = vec![create_test_entry()];
        let mut app = App::new(entries);
        app.nucleo.tick(10);

        // Set selection out of bounds
        app.selected_idx = 999;

        app.handle_action(Action::CopyToClipboard, 1);

        // Should set error status message
        assert!(app.status_message.is_some());
        let msg = app.status_message.as_ref().unwrap();
        assert_eq!(msg.text, "✗ Invalid selection");
        assert_eq!(msg.message_type, MessageType::Error);
    }

    #[test]
    fn test_handle_action_copy_to_clipboard_success() {
        let entries = vec![create_test_entry()];
        let mut app = App::new(entries);
        app.nucleo.tick(10);

        app.handle_action(Action::CopyToClipboard, 1);

        // Should set success status message (or error if clipboard unavailable)
        assert!(app.status_message.is_some());
        let msg = app.status_message.as_ref().unwrap();

        // Message could be success or clipboard error depending on environment
        if msg.message_type == MessageType::Success {
            assert_eq!(msg.text, "✓ Copied to clipboard");
        } else {
            // Clipboard might not be available in test environment
            assert!(msg.text.starts_with("✗ Clipboard error:"));
            assert_eq!(msg.message_type, MessageType::Error);
        }
    }

    #[test]
    fn test_set_status_success_message() {
        let entries = vec![create_test_entry()];
        let mut app = App::new(entries);

        app.set_status("Test success", MessageType::Success, 3000);

        assert!(app.status_message.is_some());
        let msg = app.status_message.as_ref().unwrap();
        assert_eq!(msg.text, "Test success");
        assert_eq!(msg.message_type, MessageType::Success);
        assert!(msg.expires_at > Instant::now());
    }

    #[test]
    fn test_set_status_error_message() {
        let entries = vec![create_test_entry()];
        let mut app = App::new(entries);

        app.set_status("Test error", MessageType::Error, 5000);

        assert!(app.status_message.is_some());
        let msg = app.status_message.as_ref().unwrap();
        assert_eq!(msg.text, "Test error");
        assert_eq!(msg.message_type, MessageType::Error);
        assert!(msg.expires_at > Instant::now());
    }

    #[test]
    fn test_status_message_expiry() {
        let entries = vec![create_test_entry()];
        let mut app = App::new(entries);

        // Set status with 0ms duration (already expired)
        app.set_status("Expired", MessageType::Success, 0);
        assert!(app.status_message.is_some());

        // Sleep briefly to ensure expiry time has passed
        std::thread::sleep(Duration::from_millis(1));

        // Simulate the expiry check from run loop
        let should_clear = app
            .status_message
            .as_ref()
            .map(|msg| Instant::now() >= msg.expires_at)
            .unwrap_or(false);

        assert!(should_clear);

        if should_clear {
            app.status_message = None;
        }

        assert!(app.status_message.is_none());
    }

    #[test]
    fn test_check_and_clear_expired_status_clears_expired() {
        let entries = vec![create_test_entry()];
        let mut app = App::new(entries);

        // Set status with 0ms duration (already expired)
        app.set_status("Expired", MessageType::Success, 0);
        assert!(app.status_message.is_some());

        // Sleep briefly to ensure expiry time has passed
        std::thread::sleep(Duration::from_millis(1));

        // Call the method
        app.check_and_clear_expired_status();

        // Should be cleared
        assert!(app.status_message.is_none());
    }

    #[test]
    fn test_check_and_clear_expired_status_keeps_active() {
        let entries = vec![create_test_entry()];
        let mut app = App::new(entries);

        // Set status with long duration (not expired)
        app.set_status("Active", MessageType::Success, 10000);
        assert!(app.status_message.is_some());

        // Call the method
        app.check_and_clear_expired_status();

        // Should still be present
        assert!(app.status_message.is_some());
        assert_eq!(app.status_message.as_ref().unwrap().text, "Active");
    }

    #[test]
    fn test_check_and_clear_expired_status_no_message() {
        let entries = vec![create_test_entry()];
        let mut app = App::new(entries);

        // No status message set
        assert!(app.status_message.is_none());

        // Call the method (should not panic)
        app.check_and_clear_expired_status();

        // Should still be None
        assert!(app.status_message.is_none());
    }

    #[test]
    fn test_process_nucleo_updates_returns_all_items() {
        let entries = vec![create_test_entry(), create_test_entry(), create_test_entry()];
        let mut app = App::new(entries);

        app.process_nucleo_updates();
        let matched_items = app.collect_matched_items();

        assert_eq!(matched_items.len(), 3);
    }

    #[test]
    fn test_process_nucleo_updates_with_empty_entries() {
        let mut app = App::new(vec![]);

        app.process_nucleo_updates();
        let matched_items = app.collect_matched_items();

        assert_eq!(matched_items.len(), 0);
    }

    #[test]
    fn test_process_nucleo_updates_with_search_pattern() {
        let mut entries = vec![];
        for i in 0..3 {
            let mut entry = create_test_entry();
            entry.display_text = format!("Entry {}", i);
            entries.push(entry);
        }
        let mut app = App::new(entries);

        // Set a search pattern
        app.search_query = "Entry 1".to_string();
        app.update_nucleo_pattern();

        app.process_nucleo_updates();
        let matched_items = app.collect_matched_items();

        // Should match "Entry 1" specifically
        assert_eq!(matched_items.len(), 1);
        assert!(matched_items[0].display_text.contains("Entry 1"));
    }

    #[test]
    fn test_update_nucleo_pattern_with_empty_query() {
        let entries = vec![create_test_entry(), create_test_entry()];
        let mut app = App::new(entries);

        app.search_query = "".to_string();
        app.update_nucleo_pattern();

        app.process_nucleo_updates();
        let matched_items = app.collect_matched_items();

        // Empty query should match all items
        assert_eq!(matched_items.len(), 2);
    }

    #[test]
    fn test_update_nucleo_pattern_with_pipe_separator() {
        let mut entries = vec![];
        for i in 0..3 {
            let mut entry = create_test_entry();
            entry.display_text = format!("Test {}", i);
            entries.push(entry);
        }
        let mut app = App::new(entries);

        // Query with pipe - only fuzzy portion should be used for nucleo
        app.search_query = "project:foo | Test 1".to_string();
        app.update_nucleo_pattern();

        app.process_nucleo_updates();
        let matched_items = app.collect_matched_items();

        // Should match using only "Test 1" (fuzzy portion)
        assert_eq!(matched_items.len(), 1);
        assert!(matched_items[0].display_text.contains("Test 1"));
    }

    #[test]
    fn test_update_nucleo_pattern_rapid_changes() {
        let mut entries = vec![];
        for i in 0..5 {
            let mut entry = create_test_entry();
            entry.display_text = format!("Item {}", i);
            entries.push(entry);
        }
        let mut app = App::new(entries);

        // Rapidly change pattern multiple times
        app.search_query = "Item 0".to_string();
        app.update_nucleo_pattern();

        app.search_query = "Item 1".to_string();
        app.update_nucleo_pattern();

        app.search_query = "Item 2".to_string();
        app.update_nucleo_pattern();

        app.process_nucleo_updates();
        let matched_items = app.collect_matched_items();

        // Should match final pattern "Item 2"
        assert_eq!(matched_items.len(), 1);
        assert!(matched_items[0].display_text.contains("Item 2"));
    }

    #[test]
    fn test_update_nucleo_pattern_special_characters() {
        let mut entry = create_test_entry();
        entry.display_text = "Test (special) [chars]".to_string();
        let entries = vec![entry];
        let mut app = App::new(entries);

        app.search_query = "special".to_string();
        app.update_nucleo_pattern();

        app.process_nucleo_updates();
        let matched_items = app.collect_matched_items();

        // Should match despite special characters
        assert_eq!(matched_items.len(), 1);
    }

    #[test]
    fn test_update_nucleo_pattern_case_insensitive() {
        let mut entry = create_test_entry();
        entry.display_text = "TestEntry".to_string();
        let entries = vec![entry];
        let mut app = App::new(entries);

        // Search with lowercase should match mixed case
        app.search_query = "testentry".to_string();
        app.update_nucleo_pattern();

        app.process_nucleo_updates();
        let matched_items = app.collect_matched_items();

        // Should match (Smart case matching)
        assert_eq!(matched_items.len(), 1);
    }

    #[test]
    fn test_handle_action_toggle_filter() {
        let entries = vec![create_test_entry()];
        let mut app = App::new(entries);

        // Just verify it doesn't panic (stub for Worker C)
        app.handle_action(Action::ToggleFilter, 1);
    }

    #[test]
    fn test_handle_action_toggle_focus() {
        let entries = vec![create_test_entry()];
        let mut app = App::new(entries);

        // Just verify it doesn't panic (TODO implementation)
        app.handle_action(Action::ToggleFocus, 1);
    }

    #[test]
    fn test_handle_action_refresh() {
        let entries = vec![create_test_entry()];
        let mut app = App::new(entries);

        // Just verify it doesn't panic (TODO implementation)
        app.handle_action(Action::Refresh, 1);
    }

    #[test]
    fn test_collect_matched_items_returns_all_when_no_search() {
        let entries = vec![create_test_entry(), create_test_entry(), create_test_entry()];
        let mut app = App::new(entries);

        // Nucleo needs to process items in background, tick to complete
        app.nucleo.tick(10);

        let matched = app.collect_matched_items();

        // Without search query, all items should match
        assert_eq!(matched.len(), 3);
    }

    #[test]
    fn test_collect_matched_items_with_empty_entries() {
        let mut app = App::new(vec![]);

        app.nucleo.tick(10);

        let matched = app.collect_matched_items();

        assert_eq!(matched.len(), 0);
    }

    #[test]
    fn test_handle_action_clear_search_when_empty() {
        let entries = vec![create_test_entry()];
        let mut app = App::new(entries);

        assert!(!app.should_quit);
        assert_eq!(app.search_query, "");

        app.handle_action(Action::ClearSearch, 1);

        // Should quit when search is empty
        assert!(app.should_quit);
    }

    #[test]
    fn test_handle_action_clear_search_when_active() {
        let entries = vec![create_test_entry()];
        let mut app = App::new(entries);
        app.search_query = "test query".to_string();

        assert!(!app.should_quit);

        app.handle_action(Action::ClearSearch, 1);

        // Should clear search but not quit
        assert!(!app.should_quit);
        assert_eq!(app.search_query, "");
        assert_eq!(app.selected_idx, 0);
    }

    #[test]
    fn test_search_query_length_limit() {
        let entries = vec![create_test_entry()];
        let mut app = App::new(entries);

        // Fill search query to 256 chars
        for _ in 0..256 {
            app.update_search('a');
        }

        assert_eq!(app.search_query.len(), 256);

        // Try to add one more char - should be ignored
        app.update_search('b');

        assert_eq!(app.search_query.len(), 256);
        assert!(!app.search_query.contains('b'));
    }

    #[test]
    fn test_parse_input_no_pipe() {
        let entries = vec![create_test_entry()];
        let mut app = App::new(entries);
        app.search_query = "fuzzy search".to_string();

        let (filter, fuzzy) = app.parse_input();

        assert_eq!(filter, None);
        assert_eq!(fuzzy, "fuzzy search");
    }

    #[test]
    fn test_parse_input_with_pipe() {
        let entries = vec![create_test_entry()];
        let mut app = App::new(entries);
        app.search_query = "project:foo | fuzzy".to_string();

        let (filter, fuzzy) = app.parse_input();

        assert_eq!(filter, Some("project:foo"));
        assert_eq!(fuzzy, "fuzzy");
    }

    #[test]
    fn test_parse_input_empty_filter() {
        let entries = vec![create_test_entry()];
        let mut app = App::new(entries);
        app.search_query = "| fuzzy".to_string();

        let (filter, fuzzy) = app.parse_input();

        assert_eq!(filter, None);
        assert_eq!(fuzzy, "fuzzy");
    }

    #[test]
    fn test_parse_input_empty_fuzzy() {
        let entries = vec![create_test_entry()];
        let mut app = App::new(entries);
        app.search_query = "project:foo |".to_string();

        let (filter, fuzzy) = app.parse_input();

        assert_eq!(filter, Some("project:foo"));
        assert_eq!(fuzzy, "");
    }

    #[test]
    fn test_extract_fuzzy_portion() {
        let entries = vec![create_test_entry()];
        let mut app = App::new(entries);

        app.search_query = "project:foo | tui".to_string();
        assert_eq!(app.extract_fuzzy_portion(), "tui");

        app.search_query = "no pipe here".to_string();
        assert_eq!(app.extract_fuzzy_portion(), "no pipe here");
    }

    #[test]
    fn test_extract_filter_portion() {
        let entries = vec![create_test_entry()];
        let mut app = App::new(entries);

        app.search_query = "project:foo | tui".to_string();
        assert_eq!(app.extract_filter_portion(), Some("project:foo".to_string()));

        app.search_query = "no pipe here".to_string();
        assert_eq!(app.extract_filter_portion(), None);
    }

    #[test]
    fn test_apply_filter_with_valid_filter() {
        let mut entries = vec![create_test_entry()];
        entries[0].entry_type = crate::models::EntryType::UserPrompt;
        let mut app = App::new(entries.clone());

        // Set up filter query
        app.search_query = "type:user | test".to_string();
        app.apply_filter();

        // Should have applied filter successfully
        assert!(app.filter_error.is_none());
        assert!(app.current_filter.is_some());
        assert_eq!(app.filtered_entries.len(), 1);
    }

    #[test]
    fn test_apply_filter_with_parse_error() {
        let entries = vec![create_test_entry()];
        let mut app = App::new(entries);

        // Invalid filter syntax
        app.search_query = "invalid::: | test".to_string();
        app.apply_filter();

        // Should have parse error
        assert!(app.filter_error.is_some());
        assert!(app.filter_error.as_ref().unwrap().contains("Parse error"));
    }

    #[test]
    fn test_apply_filter_reset_with_no_filter() {
        let entries = vec![create_test_entry(), create_test_entry()];
        let mut app = App::new(entries.clone());

        // First apply a filter
        app.search_query = "type:user | test".to_string();
        app.apply_filter();

        // Now reset by removing filter portion
        app.search_query = "test".to_string();
        app.apply_filter();

        // Should reset to all entries
        assert!(app.filter_error.is_none());
        assert!(app.current_filter.is_none());
        assert_eq!(app.filtered_entries.len(), 2);
    }

    #[test]
    fn test_apply_filter_with_empty_filter() {
        let entries = vec![create_test_entry(), create_test_entry()];
        let mut app = App::new(entries.clone());

        // Empty filter portion (just pipe)
        app.search_query = "| fuzzy".to_string();
        app.apply_filter();

        // Should reset to all entries
        assert!(app.filter_error.is_none());
        assert!(app.current_filter.is_none());
        assert_eq!(app.filtered_entries.len(), 2);
    }

    #[test]
    fn test_re_inject_entries_after_filter() {
        let mut entries = vec![];
        for i in 0..5 {
            let mut entry = create_test_entry();
            entry.display_text = format!("Entry {}", i);
            entries.push(entry);
        }
        let mut app = App::new(entries);

        // Apply a filter
        app.search_query = "type:user | Entry".to_string();
        app.apply_filter();

        // Tick nucleo to process
        app.nucleo.tick(10);

        // Verify entries were re-injected
        let matched = app.collect_matched_items();
        assert_eq!(matched.len(), 5);
    }

    #[test]
    fn test_handle_action_apply_filter() {
        let entries = vec![create_test_entry()];
        let mut app = App::new(entries);

        app.search_query = "type:user | test".to_string();
        app.handle_action(Action::ApplyFilter, 1);

        // Filter should be applied
        assert!(app.last_enter_time.is_some());
    }

    #[test]
    fn test_handle_action_apply_filter_debounce() {
        let entries = vec![create_test_entry()];
        let mut app = App::new(entries);

        app.search_query = "type:user | test".to_string();

        // First apply
        app.handle_action(Action::ApplyFilter, 1);
        assert!(app.last_enter_time.is_some());

        // Immediate second apply (should be debounced)
        let first_time = app.last_enter_time;
        app.handle_action(Action::ApplyFilter, 1);

        // Time should not have changed much (debounced)
        assert_eq!(app.last_enter_time, first_time);
    }

    // End-to-end TUI filter workflow tests
    #[test]
    fn test_tui_filter_workflow_valid_filter() {
        // Create test data with different entry types
        let mut entries = vec![create_test_entry(), create_test_entry(), create_test_entry()];
        entries[0].entry_type = crate::models::EntryType::UserPrompt;
        entries[0].project_path = Some("/Users/test/project1".into());
        entries[1].entry_type = crate::models::EntryType::AgentMessage;
        entries[1].project_path = Some("/Users/test/project1".into());
        entries[2].entry_type = crate::models::EntryType::UserPrompt;
        entries[2].project_path = Some("/Users/test/project2".into());

        let mut app = App::new(entries);

        // Simulate user typing a filter query: "type:user | search"
        for c in "type:user | search".chars() {
            app.handle_action(Action::UpdateSearch(c), 0);
        }

        // Apply filter
        app.handle_action(Action::ApplyFilter, 0);

        // Verify filter was applied (no error)
        assert!(app.filter_error.is_none());
        assert!(app.current_filter.is_some());

        // Verify only user entries remain
        assert_eq!(app.filtered_entries.len(), 2);
        assert!(
            app.filtered_entries
                .iter()
                .all(|e| matches!(e.entry_type, crate::models::EntryType::UserPrompt))
        );
    }

    #[test]
    fn test_tui_filter_workflow_parse_error() {
        let entries = vec![create_test_entry()];
        let mut app = App::new(entries);

        // Type invalid filter
        for c in "invalid::: | search".chars() {
            app.handle_action(Action::UpdateSearch(c), 0);
        }

        // Apply filter
        app.handle_action(Action::ApplyFilter, 0);

        // Verify parse error was set
        assert!(app.filter_error.is_some());
        assert!(app.filter_error.as_ref().unwrap().contains("Parse error"));
    }

    #[test]
    fn test_tui_filter_workflow_reset() {
        let mut entries = vec![create_test_entry(), create_test_entry()];
        entries[0].entry_type = crate::models::EntryType::UserPrompt;
        entries[1].entry_type = crate::models::EntryType::AgentMessage;

        let mut app = App::new(entries);

        // Apply filter first
        app.search_query = "type:user | search".to_string();
        app.apply_filter();

        // Verify filter is active
        assert_eq!(app.filtered_entries.len(), 1);
        assert!(app.current_filter.is_some());

        // Remove filter by updating search query to have no pipe
        app.search_query = "search".to_string();
        app.apply_filter();

        // Verify filter was reset
        assert!(app.current_filter.is_none());
        assert!(app.filter_error.is_none());
        assert_eq!(app.filtered_entries.len(), 2); // All entries restored
    }

    #[test]
    fn test_tui_filter_workflow_combined_filters() {
        let mut entries = vec![create_test_entry(), create_test_entry(), create_test_entry()];
        entries[0].entry_type = crate::models::EntryType::UserPrompt;
        entries[0].project_path = Some("/Users/test/project1".into());
        entries[1].entry_type = crate::models::EntryType::AgentMessage;
        entries[1].project_path = Some("/Users/test/project1".into());
        entries[2].entry_type = crate::models::EntryType::UserPrompt;
        entries[2].project_path = Some("/Users/test/project2".into());

        let mut app = App::new(entries);

        // Apply combined filter
        for c in "project:project1 type:user | fuzzy".chars() {
            app.handle_action(Action::UpdateSearch(c), 0);
        }
        app.handle_action(Action::ApplyFilter, 0);

        // Verify combined filter was applied
        assert!(app.filter_error.is_none());
        assert_eq!(app.filtered_entries.len(), 1);
        assert!(
            app.filtered_entries[0]
                .project_path
                .as_ref()
                .unwrap()
                .to_string_lossy()
                .contains("project1")
        );
        assert!(matches!(app.filtered_entries[0].entry_type, crate::models::EntryType::UserPrompt));
    }

    #[test]
    fn test_filter_with_fuzzy_search_integration() {
        let mut entries = vec![];
        for i in 0..5 {
            let mut entry = create_test_entry();
            entry.display_text = format!("Test entry {}", i);
            entry.entry_type = crate::models::EntryType::UserPrompt;
            entries.push(entry);
        }
        let mut app = App::new(entries);

        // Apply filter + fuzzy search
        app.search_query = "type:user | Test entry 2".to_string();
        app.apply_filter();

        app.process_nucleo_updates();
        let matched_items = app.collect_matched_items();

        // Should match only "Test entry 2" after both filter and fuzzy
        assert_eq!(matched_items.len(), 1);
        assert!(matched_items[0].display_text.contains("Test entry 2"));
    }

    #[test]
    fn test_filter_change_updates_nucleo() {
        let mut entries = vec![create_test_entry(), create_test_entry()];
        entries[0].entry_type = crate::models::EntryType::UserPrompt;
        entries[0].display_text = "User entry".to_string();
        entries[1].entry_type = crate::models::EntryType::AgentMessage;
        entries[1].display_text = "Agent entry".to_string();
        let mut app = App::new(entries);

        // First filter: user entries only
        app.search_query = "type:user | entry".to_string();
        app.apply_filter();
        app.process_nucleo_updates();
        let matched_items = app.collect_matched_items();
        assert_eq!(matched_items.len(), 1);
        assert!(matched_items[0].display_text.contains("User"));

        // Change filter: agent entries only
        app.search_query = "type:agent | entry".to_string();
        app.apply_filter();
        app.process_nucleo_updates();
        let matched_items = app.collect_matched_items();
        assert_eq!(matched_items.len(), 1);
        assert!(matched_items[0].display_text.contains("Agent"));
    }

    #[test]
    fn test_re_inject_entries_preserves_fuzzy_pattern() {
        let mut entries = vec![];
        for i in 0..3 {
            let mut entry = create_test_entry();
            entry.display_text = format!("Item {}", i);
            entries.push(entry);
        }
        let mut app = App::new(entries);

        // Set fuzzy pattern first
        app.search_query = "type:user | Item 1".to_string();
        app.apply_filter();

        app.process_nucleo_updates();
        let matched_items = app.collect_matched_items();

        // After re-injection, fuzzy pattern should still be active
        assert_eq!(matched_items.len(), 1);
        assert!(matched_items[0].display_text.contains("Item 1"));
    }

    #[test]
    fn test_multiple_filter_applications() {
        let mut entries = vec![];
        for i in 0..10 {
            let mut entry = create_test_entry();
            entry.display_text = format!("Entry {}", i);
            entry.entry_type = if i % 2 == 0 {
                crate::models::EntryType::UserPrompt
            } else {
                crate::models::EntryType::AgentMessage
            };
            entries.push(entry);
        }
        let mut app = App::new(entries);

        // Apply filter multiple times
        app.search_query = "type:user | Entry".to_string();
        app.apply_filter();
        assert_eq!(app.filtered_entries.len(), 5);

        app.search_query = "| Entry".to_string();
        app.apply_filter();
        assert_eq!(app.filtered_entries.len(), 10); // Reset to all

        app.search_query = "type:agent | Entry".to_string();
        app.apply_filter();
        assert_eq!(app.filtered_entries.len(), 5);
    }

    #[test]
    fn test_re_inject_resets_selection() {
        let mut entries = vec![];
        for i in 0..5 {
            let mut entry = create_test_entry();
            entry.display_text = format!("Entry {}", i);
            entries.push(entry);
        }
        let mut app = App::new(entries);

        // Select item
        app.selected_idx = 3;

        // Re-inject entries
        app.re_inject_entries();

        // Selection should be reset to 0
        assert_eq!(app.selected_idx, 0);
    }

    #[test]
    fn test_filter_workflow_empty_result() {
        let mut entries = vec![create_test_entry()];
        entries[0].entry_type = crate::models::EntryType::UserPrompt;
        entries[0].display_text = "User entry".to_string();
        let mut app = App::new(entries);

        // Apply filter that matches nothing
        app.search_query = "type:agent | test".to_string();
        app.apply_filter();

        app.process_nucleo_updates();
        let matched_items = app.collect_matched_items();

        // Should have zero matches
        assert_eq!(matched_items.len(), 0);
    }

    // Edge case tests
    #[test]
    fn test_search_query_boundary_255_chars() {
        let entries = vec![create_test_entry()];
        let mut app = App::new(entries);

        // Fill to exactly 255 chars
        for _ in 0..255 {
            app.update_search('a');
        }

        assert_eq!(app.search_query.len(), 255);

        // Should be able to add one more to reach 256
        app.update_search('b');
        assert_eq!(app.search_query.len(), 256);
    }

    #[test]
    fn test_search_query_boundary_at_256_chars() {
        let entries = vec![create_test_entry()];
        let mut app = App::new(entries);

        // Fill to exactly 256 chars
        for _ in 0..256 {
            app.update_search('a');
        }

        assert_eq!(app.search_query.len(), 256);

        // Should NOT be able to add more (at limit)
        app.update_search('b');
        assert_eq!(app.search_query.len(), 256);
        assert!(!app.search_query.contains('b'));
    }

    #[test]
    fn test_search_query_boundary_257_attempts() {
        let entries = vec![create_test_entry()];
        let mut app = App::new(entries);

        // Try to add 257 chars
        for _ in 0..257 {
            app.update_search('a');
        }

        // Should be capped at 256
        assert_eq!(app.search_query.len(), 256);
    }

    #[test]
    fn test_dirty_state_tracking_on_status_set() {
        let entries = vec![create_test_entry()];
        let mut app = App::new(entries);

        // Set status should mark dirty
        app.needs_redraw = false;
        app.set_status("Test", MessageType::Success, 1000);
        assert!(app.needs_redraw, "Setting status should mark dirty");
    }

    #[test]
    fn test_dirty_state_on_search_operations() {
        let entries = vec![create_test_entry()];
        let mut app = App::new(entries);

        // Update search should mark dirty
        app.needs_redraw = false;
        app.update_search('a');
        assert!(app.needs_redraw, "Update search should mark dirty");

        // Delete char should mark dirty
        app.needs_redraw = false;
        app.delete_char();
        assert!(app.needs_redraw, "Delete char should mark dirty");

        // Delete from empty should not mark dirty
        app.search_query.clear();
        app.needs_redraw = false;
        app.delete_char();
        assert!(!app.needs_redraw, "Delete from empty should not mark dirty");
    }

    #[test]
    fn test_dirty_state_on_selection_move() {
        let entries = vec![create_test_entry(), create_test_entry()];
        let mut app = App::new(entries);

        // Move selection should mark dirty when index changes
        app.needs_redraw = false;
        app.move_selection(1, 2);
        assert!(app.needs_redraw, "Move selection should mark dirty");
        assert_eq!(app.selected_idx, 1);

        // No movement should not mark dirty (at bounds)
        app.needs_redraw = false;
        app.move_selection(1, 2); // Try to go past end
        assert!(!app.needs_redraw, "No movement should not mark dirty");
        assert_eq!(app.selected_idx, 1); // Still at same position
    }

    #[test]
    fn test_dirty_state_on_clear_search() {
        let entries = vec![create_test_entry()];
        let mut app = App::new(entries);
        app.search_query = "test".to_string();

        // Clear search should mark dirty
        app.needs_redraw = false;
        app.handle_action(Action::ClearSearch, 1);
        assert!(app.needs_redraw, "Clear search should mark dirty");
        assert_eq!(app.search_query, "", "Search should be cleared");
    }

    #[test]
    fn test_needs_redraw_initial_state() {
        let entries = vec![create_test_entry()];
        let app = App::new(entries);

        // Should need redraw initially
        assert!(app.needs_redraw, "Should need initial draw");
    }

    #[test]
    fn test_move_selection_with_empty_results() {
        let entries = vec![create_test_entry()];
        let mut app = App::new(entries);

        // Move when total is 0
        app.move_selection(1, 0);
        assert_eq!(app.selected_idx, 0);

        app.move_selection(-1, 0);
        assert_eq!(app.selected_idx, 0);
    }

    #[test]
    fn test_handle_action_with_empty_state() {
        let mut app = App::new(vec![]);

        // All actions should work with empty state
        app.handle_action(Action::MoveUp, 0);
        app.handle_action(Action::MoveDown, 0);
        app.handle_action(Action::PageUp, 0);
        app.handle_action(Action::PageDown, 0);
        app.handle_action(Action::UpdateSearch('a'), 0);
        app.handle_action(Action::DeleteChar, 0);
        app.handle_action(Action::ClearSearch, 0);

        // Should not crash
    }

    #[test]
    fn test_apply_filter_with_empty_entries() {
        let mut app = App::new(vec![]);

        app.search_query = "type:user | test".to_string();
        app.apply_filter();

        // Should handle gracefully
        assert_eq!(app.filtered_entries.len(), 0);
        assert!(app.filter_error.is_none());
    }

    #[test]
    fn test_multiple_concurrent_status_messages() {
        let entries = vec![create_test_entry()];
        let mut app = App::new(entries);

        // Set first status
        app.set_status("First message", MessageType::Success, 5000);
        assert_eq!(app.status_message.as_ref().unwrap().text, "First message");

        // Set second status (should replace first)
        app.set_status("Second message", MessageType::Error, 3000);
        assert_eq!(app.status_message.as_ref().unwrap().text, "Second message");
        assert_eq!(app.status_message.as_ref().unwrap().message_type, MessageType::Error);
    }

    #[test]
    fn test_status_message_replacement() {
        let entries = vec![create_test_entry()];
        let mut app = App::new(entries);

        // Success message
        app.set_status("Success", MessageType::Success, 10000);
        assert_eq!(app.status_message.as_ref().unwrap().message_type, MessageType::Success);

        // Immediately replace with error
        app.set_status("Error", MessageType::Error, 10000);
        assert_eq!(app.status_message.as_ref().unwrap().text, "Error");
        assert_eq!(app.status_message.as_ref().unwrap().message_type, MessageType::Error);
    }

    #[test]
    fn test_empty_search_query_after_deletion() {
        let entries = vec![create_test_entry()];
        let mut app = App::new(entries);

        app.update_search('a');
        assert_eq!(app.search_query, "a");

        app.delete_char();
        assert_eq!(app.search_query, "");

        // Deleting from empty should not crash
        app.delete_char();
        assert_eq!(app.search_query, "");
    }

    #[test]
    fn test_collect_matched_items_timing() {
        let entries = vec![create_test_entry(), create_test_entry()];
        let app = App::new(entries);

        // Call multiple times in succession
        let matched1 = app.collect_matched_items();
        let matched2 = app.collect_matched_items();
        let matched3 = app.collect_matched_items();

        // Should return consistent results
        assert_eq!(matched1.len(), matched2.len());
        assert_eq!(matched2.len(), matched3.len());
    }

    #[test]
    fn test_nucleo_pattern_with_empty_fuzzy_portion() {
        let entries = vec![create_test_entry(), create_test_entry()];
        let mut app = App::new(entries);

        // Query with filter but empty fuzzy portion
        app.search_query = "type:user |".to_string();
        app.update_nucleo_pattern();

        app.process_nucleo_updates();
        let matched_items = app.collect_matched_items();

        // Empty fuzzy should match all (after filter is applied)
        assert_eq!(matched_items.len(), 2);
    }

    #[test]
    fn test_move_selection_single_item() {
        let entries = vec![create_test_entry()];
        let mut app = App::new(entries);

        // With only 1 item, selection should stay at 0
        app.move_selection(10, 1);
        assert_eq!(app.selected_idx, 0);

        app.move_selection(-10, 1);
        assert_eq!(app.selected_idx, 0);
    }
}
