use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use nucleo::{Config, Nucleo};
use ratatui::Terminal;
use ratatui::backend::Backend;

use super::events::{Action, poll_event};
use super::rendering::render_ui;
use crate::models::SearchEntry;

pub struct App {
    nucleo: Nucleo<SearchEntry>,
    selected_idx: usize,
    search_query: String,
    should_quit: bool,
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

        Self { nucleo, selected_idx: 0, search_query: String::new(), should_quit: false }
    }

    pub fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        while !self.should_quit {
            // Get latest match results from nucleo
            let matched_items = self.collect_matched_items();

            // Render UI
            terminal.draw(|f| {
                render_ui(f, &matched_items, self.selected_idx, &self.search_query);
            })?;

            // Handle events
            let action = poll_event(Duration::from_millis(100))?;
            self.handle_action(action, matched_items.len());
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
            Action::MoveUp => self.move_selection(-1, total_items),
            Action::MoveDown => self.move_selection(1, total_items),
            Action::PageUp => self.move_selection(-10, total_items),
            Action::PageDown => self.move_selection(10, total_items),
            Action::UpdateSearch(c) => self.update_search(c),
            Action::DeleteChar => self.delete_char(),
            Action::CopyToClipboard => {
                // Stub for Worker B (clipboard integration)
                eprintln!("TODO: Copy to clipboard");
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
        self.search_query.push(c);
        self.update_nucleo_pattern();
        self.selected_idx = 0; // Reset selection on search change
    }

    fn delete_char(&mut self) {
        self.search_query.pop();
        self.update_nucleo_pattern();
        self.selected_idx = 0;
    }

    fn update_nucleo_pattern(&mut self) {
        self.nucleo.pattern.reparse(
            0,
            &self.search_query,
            nucleo::pattern::CaseMatching::Smart,
            nucleo::pattern::Normalization::Smart,
            false,
        );
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
}
