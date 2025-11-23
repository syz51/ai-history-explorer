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
        }
    }

    /// Set a transient status message with automatic expiry
    fn set_status(&mut self, text: impl Into<String>, message_type: MessageType, duration_ms: u64) {
        self.status_message = Some(StatusMessage {
            text: text.into(),
            message_type,
            expires_at: Instant::now() + Duration::from_millis(duration_ms),
        });
    }

    pub fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        while !self.should_quit {
            // Tick nucleo to process matches
            self.nucleo.tick(10);

            // Clear expired status messages (before borrowing self)
            let should_clear = self
                .status_message
                .as_ref()
                .map(|msg| Instant::now() >= msg.expires_at)
                .unwrap_or(false);
            if should_clear {
                self.status_message = None;
            }

            // Get latest match results from nucleo
            let matched_items = self.collect_matched_items();
            let matched_count = matched_items.len();

            // Render UI
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
                    self.set_status("✗ No entries to copy", MessageType::Error, 3000);
                } else if self.selected_idx >= matched_items.len() {
                    self.set_status("✗ Invalid selection", MessageType::Error, 3000);
                } else {
                    // Copy selected entry's display text
                    let entry = matched_items[self.selected_idx];
                    match copy_to_clipboard(&entry.display_text) {
                        Ok(()) => {
                            self.set_status("✓ Copied to clipboard", MessageType::Success, 3000);
                        }
                        Err(e) => {
                            self.set_status(
                                format!("✗ Clipboard error: {}", e),
                                MessageType::Error,
                                5000,
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

        let new_idx = (self.selected_idx as isize + delta).max(0) as usize;
        self.selected_idx = new_idx.min(total - 1);
    }

    fn update_search(&mut self, c: char) {
        // Limit search query to 256 characters to prevent DoS
        if self.search_query.len() < 256 {
            self.search_query.push(c);
            self.update_nucleo_pattern();
            self.selected_idx = 0; // Reset selection on search change
        }
    }

    fn delete_char(&mut self) {
        self.search_query.pop();
        self.update_nucleo_pattern();
        self.selected_idx = 0;
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
                    }
                    Err(e) => {
                        self.filter_error = Some(format!("Filter error: {}", e));
                    }
                }
            }
            Err(e) => {
                self.filter_error =
                    Some(format!("Parse error: {} | Try: project:name type:user | search", e));
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
    fn test_handle_action_copy_to_clipboard() {
        let entries = vec![create_test_entry()];
        let mut app = App::new(entries);

        // Just verify it doesn't panic (stub for Worker B)
        app.handle_action(Action::CopyToClipboard, 1);
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
}
