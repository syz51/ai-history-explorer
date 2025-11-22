use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};

use super::layout::AppLayout;
use super::timestamps::format_timestamp;
use crate::models::{EntryType, SearchEntry};
use crate::utils::format_path_with_tilde;

/// Render the entire UI
pub fn render_ui(
    frame: &mut Frame,
    entries: &[&SearchEntry],
    selected_idx: usize,
    search_query: &str,
) {
    let layout = AppLayout::new(frame.area());

    render_results_list(frame, layout.results_area, entries, selected_idx);
    render_preview(frame, layout.preview_area, entries.get(selected_idx).copied());
    render_status_bar(frame, layout.status_area, entries.len(), selected_idx, search_query);
}

fn render_results_list(
    frame: &mut Frame,
    area: Rect,
    entries: &[&SearchEntry],
    selected_idx: usize,
) {
    let items: Vec<ListItem> = entries
        .iter()
        .enumerate()
        .map(|(idx, entry)| {
            let icon = match entry.entry_type {
                EntryType::UserPrompt => "👤",
                EntryType::AgentMessage => "🤖",
            };

            let timestamp = format_timestamp(&entry.timestamp);
            let project = entry
                .project_path
                .as_ref()
                .map(|p| format_path_with_tilde(p))
                .unwrap_or_else(|| "global".to_string());

            // Truncate display text for list view (first line only)
            let preview_text = entry
                .display_text
                .lines()
                .next()
                .unwrap_or("")
                .chars()
                .take(50)
                .collect::<String>();

            let content = format!("{} {} | {} | {}", icon, timestamp, project, preview_text);

            let style = if idx == selected_idx {
                Style::default()
                    .fg(Color::Rgb(250, 250, 250)) // Bright text
                    .bg(Color::Rgb(16, 185, 129)) // Emerald background
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Rgb(113, 113, 122)) // Muted text
            };

            ListItem::new(content).style(style)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(113, 113, 122)))
            .title(" Results "),
    );

    frame.render_widget(list, area);
}

fn render_preview(frame: &mut Frame, area: Rect, entry: Option<&SearchEntry>) {
    let content = if let Some(entry) = entry {
        let timestamp = format_timestamp(&entry.timestamp);
        let project = entry
            .project_path
            .as_ref()
            .map(|p| format_path_with_tilde(p))
            .unwrap_or_else(|| "global".to_string());
        let session_id = entry.session_id.clone();

        let mut lines = vec![
            Line::from(vec![
                Span::styled("Timestamp: ", Style::default().fg(Color::Rgb(113, 113, 122))),
                Span::raw(timestamp),
            ]),
            Line::from(vec![
                Span::styled("Project: ", Style::default().fg(Color::Rgb(113, 113, 122))),
                Span::raw(project),
            ]),
            Line::from(vec![
                Span::styled("Session: ", Style::default().fg(Color::Rgb(113, 113, 122))),
                Span::raw(session_id),
            ]),
            Line::from(""),
        ];

        // Add display text (already truncated by SearchEntry)
        for line in entry.display_text.lines() {
            lines.push(Line::from(line));
        }

        Text::from(lines)
    } else {
        Text::from("No entry selected")
    };

    let paragraph = Paragraph::new(content)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Rgb(113, 113, 122)))
                .title(" Preview "),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, area);
}

fn render_status_bar(
    frame: &mut Frame,
    area: Rect,
    total_entries: usize,
    selected_idx: usize,
    search_query: &str,
) {
    let status_text = if search_query.is_empty() {
        format!(
            " Showing {} entries | Entry {}/{} | Enter: copy | /: filter | q: quit ",
            total_entries,
            selected_idx + 1,
            total_entries
        )
    } else {
        format!(
            " Search: {} | {} results | Entry {}/{} | Esc: clear | Enter: copy ",
            search_query,
            total_entries,
            selected_idx + 1,
            total_entries
        )
    };

    let paragraph = Paragraph::new(status_text).style(
        Style::default().fg(Color::Rgb(250, 250, 250)).bg(Color::Rgb(24, 24, 27)), // Dark zinc
    );

    frame.render_widget(paragraph, area);
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;

    fn create_test_entry(text: &str) -> SearchEntry {
        SearchEntry {
            entry_type: EntryType::UserPrompt,
            display_text: text.to_string(),
            timestamp: Utc.timestamp_opt(1234567890, 0).unwrap(),
            project_path: None,
            session_id: "test-session".to_string(),
        }
    }

    #[test]
    fn test_render_ui_with_entries() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();

        let entries = [create_test_entry("First entry"), create_test_entry("Second entry")];
        let entry_refs: Vec<&SearchEntry> = entries.iter().collect();

        terminal
            .draw(|f| {
                render_ui(f, &entry_refs, 0, "test");
            })
            .unwrap();

        // Just verify it doesn't panic
    }

    #[test]
    fn test_render_ui_empty_entries() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();

        let entries: Vec<&SearchEntry> = vec![];

        terminal
            .draw(|f| {
                render_ui(f, &entries, 0, "");
            })
            .unwrap();
    }

    #[test]
    fn test_render_preview_with_entry() {
        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        let entry = create_test_entry("Test content");

        terminal
            .draw(|f| {
                let area = f.area();
                render_preview(f, area, Some(&entry));
            })
            .unwrap();
    }

    #[test]
    fn test_render_preview_no_entry() {
        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let area = f.area();
                render_preview(f, area, None);
            })
            .unwrap();
    }

    #[test]
    fn test_render_results_list_with_project_path() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();

        let mut entry = create_test_entry("Entry with path");
        entry.project_path = Some(std::path::PathBuf::from("/Users/test/project"));

        let entries = vec![&entry];

        terminal
            .draw(|f| {
                let area = f.area();
                render_results_list(f, area, &entries, 0);
            })
            .unwrap();
    }

    #[test]
    fn test_render_results_list_agent_message() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();

        let mut entry = create_test_entry("Agent response");
        entry.entry_type = EntryType::AgentMessage;

        let entries = vec![&entry];

        terminal
            .draw(|f| {
                let area = f.area();
                render_results_list(f, area, &entries, 0);
            })
            .unwrap();
    }

    #[test]
    fn test_render_status_bar_with_search() {
        let backend = TestBackend::new(100, 1);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let area = f.area();
                render_status_bar(f, area, 10, 5, "search query");
            })
            .unwrap();
    }

    #[test]
    fn test_render_status_bar_no_search() {
        let backend = TestBackend::new(100, 1);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let area = f.area();
                render_status_bar(f, area, 10, 0, "");
            })
            .unwrap();
    }

    #[test]
    fn test_render_preview_multiline_content() {
        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        let entry = create_test_entry("Line 1\nLine 2\nLine 3");

        terminal
            .draw(|f| {
                let area = f.area();
                render_preview(f, area, Some(&entry));
            })
            .unwrap();
    }
}
