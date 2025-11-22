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
            let snapshot = self.nucleo.snapshot();
            let matched_items: Vec<&SearchEntry> = snapshot
                .matched_items(..snapshot.matched_item_count())
                .map(|item| item.data)
                .collect();

            // Render UI
            terminal.draw(|f| {
                render_ui(f, &matched_items, self.selected_idx, &self.search_query);
            })?;

            // Handle events
            match poll_event(Duration::from_millis(100))? {
                Action::Quit => self.should_quit = true,
                Action::MoveUp => self.move_selection(-1, matched_items.len()),
                Action::MoveDown => self.move_selection(1, matched_items.len()),
                Action::PageUp => self.move_selection(-10, matched_items.len()),
                Action::PageDown => self.move_selection(10, matched_items.len()),
                Action::UpdateSearch(c) => self.update_search(c),
                Action::DeleteChar => self.delete_char(),
                Action::CopyToClipboard => self.copy_to_clipboard(&matched_items),
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

        Ok(())
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

    fn copy_to_clipboard(&self, matched_items: &[&SearchEntry]) {
        // Stub for Worker B (clipboard integration)
        if let Some(entry) = matched_items.get(self.selected_idx) {
            eprintln!("TODO: Copy to clipboard: {}", entry.display_text);
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;

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
}
