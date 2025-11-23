use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};

use super::app::{MessageType, StatusMessage};
use super::layout::AppLayout;
use super::timestamps::format_timestamp;
use crate::models::{EntryType, SearchEntry};
use crate::utils::format_path_with_tilde;

/// App state needed for rendering
pub struct RenderState<'a> {
    pub search_query: &'a str,
    pub filtered_count: usize,
    pub total_count: usize,
    pub filter_error: Option<&'a str>,
    pub status_message: Option<&'a StatusMessage>,
}

/// Status bar entry counts
struct StatusCounts {
    matched: usize,
    filtered: usize,
    total: usize,
}

/// Render the entire UI
pub fn render_ui(
    frame: &mut Frame,
    entries: &[&SearchEntry],
    selected_idx: usize,
    state: &RenderState,
) {
    let layout = AppLayout::new(frame.area());

    render_results_list(frame, layout.results_area, entries, selected_idx);
    render_preview(frame, layout.preview_area, entries.get(selected_idx).copied());
    render_status_bar(
        frame,
        layout.status_area,
        StatusCounts {
            matched: entries.len(),
            filtered: state.filtered_count,
            total: state.total_count,
        },
        selected_idx,
        state.search_query,
        state.filter_error,
        state.status_message,
    );
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
    counts: StatusCounts,
    selected_idx: usize,
    search_query: &str,
    filter_error: Option<&str>,
    status_message: Option<&StatusMessage>,
) {
    // Parse input to extract filter portion
    let (filter_part, fuzzy_part) = if let Some(pipe_pos) = search_query.find('|') {
        let filter = search_query[..pipe_pos].trim();
        let fuzzy = search_query[pipe_pos + 1..].trim();
        (if filter.is_empty() { None } else { Some(filter) }, fuzzy)
    } else {
        (None, search_query)
    };

    let (status_text, style) = if let Some(msg) = status_message {
        // Show status message with appropriate color
        let (fg, bg) = match msg.message_type {
            MessageType::Success => (Color::Rgb(16, 185, 129), Color::Rgb(24, 24, 27)), // Green
            MessageType::Error => (Color::Rgb(239, 68, 68), Color::Rgb(24, 24, 27)),    // Red
        };
        (format!(" {} ", msg.text), Style::default().fg(fg).bg(bg))
    } else if let Some(error) = filter_error {
        // Show error in red
        (
            format!(" [ERROR] {} ", error),
            Style::default().fg(Color::Rgb(239, 68, 68)).bg(Color::Rgb(24, 24, 27)), // Red
        )
    } else if counts.matched == 0 {
        (
            " No entries | Enter: apply filter | Esc: clear | Ctrl+C: quit ".to_string(),
            Style::default().fg(Color::Rgb(250, 250, 250)).bg(Color::Rgb(24, 24, 27)),
        )
    } else {
        let mut parts = vec![];

        // Mode indicator
        parts.push("[FUZZY]".to_string());

        // Match counts: matched/filtered (total)
        if counts.filtered < counts.total {
            parts.push(format!("{}/{} ({} total)", counts.matched, counts.filtered, counts.total));
        } else {
            // No filter active, just show matched/total
            parts.push(format!("{}/{} total", counts.matched, counts.total));
        }

        // Active filter
        if let Some(filter) = filter_part {
            parts.push(format!("filter: {}", filter));
        }

        // Current selection
        if counts.matched > 0 {
            parts.push(format!("entry {}/{}", selected_idx + 1, counts.matched));
        }

        // Keybindings
        if !fuzzy_part.is_empty() {
            parts.push("Esc: clear".to_string());
        }
        parts.push("Enter: apply".to_string());
        parts.push("Ctrl+Y: copy".to_string());
        parts.push("Ctrl+C: quit".to_string());

        (
            format!(" {} ", parts.join(" | ")),
            Style::default().fg(Color::Rgb(250, 250, 250)).bg(Color::Rgb(24, 24, 27)),
        )
    };

    let paragraph = Paragraph::new(status_text).style(style);

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
                let state = RenderState {
                    search_query: "test",
                    filtered_count: 2,
                    total_count: 2,
                    filter_error: None,
                    status_message: None,
                };
                render_ui(f, &entry_refs, 0, &state);
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
                let state = RenderState {
                    search_query: "",
                    filtered_count: 0,
                    total_count: 0,
                    filter_error: None,
                    status_message: None,
                };
                render_ui(f, &entries, 0, &state);
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
                render_status_bar(
                    f,
                    area,
                    StatusCounts { matched: 10, filtered: 10, total: 10 },
                    5,
                    "search query",
                    None,
                    None,
                );
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
                render_status_bar(
                    f,
                    area,
                    StatusCounts { matched: 10, filtered: 10, total: 10 },
                    0,
                    "",
                    None,
                    None,
                );
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

    #[test]
    fn test_render_status_bar_empty_entries() {
        let backend = TestBackend::new(100, 1);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let area = f.area();
                render_status_bar(
                    f,
                    area,
                    StatusCounts { matched: 0, filtered: 0, total: 0 },
                    0,
                    "",
                    None,
                    None,
                );
            })
            .unwrap();
    }

    #[test]
    fn test_render_status_bar_with_filter_error() {
        let backend = TestBackend::new(100, 1);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let area = f.area();
                render_status_bar(
                    f,
                    area,
                    StatusCounts { matched: 10, filtered: 10, total: 10 },
                    0,
                    "test query",
                    Some("Parse error: invalid filter"),
                    None,
                );
            })
            .unwrap();
    }

    #[test]
    fn test_render_status_bar_with_filter_portion() {
        let backend = TestBackend::new(100, 1);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let area = f.area();
                render_status_bar(
                    f,
                    area,
                    StatusCounts { matched: 5, filtered: 8, total: 10 },
                    0,
                    "type:user | search",
                    None,
                    None,
                );
            })
            .unwrap();
    }

    #[test]
    fn test_render_status_bar_with_filtered_count() {
        let backend = TestBackend::new(100, 1);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let area = f.area();
                // matched_count=5, filtered_count=8, total_count=10
                render_status_bar(
                    f,
                    area,
                    StatusCounts { matched: 5, filtered: 8, total: 10 },
                    0,
                    "search",
                    None,
                    None,
                );
            })
            .unwrap();
    }

    #[test]
    fn test_render_ui_with_filter_error() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();

        let entries = [create_test_entry("First entry")];
        let entry_refs: Vec<&SearchEntry> = entries.iter().collect();

        terminal
            .draw(|f| {
                let state = RenderState {
                    search_query: "invalid::: | test",
                    filtered_count: 1,
                    total_count: 1,
                    filter_error: Some("Filter parse error"),
                    status_message: None,
                };
                render_ui(f, &entry_refs, 0, &state);
            })
            .unwrap();
    }

    #[test]
    fn test_render_status_bar_with_empty_fuzzy_part() {
        let backend = TestBackend::new(100, 1);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let area = f.area();
                // Empty fuzzy part should not show "Esc: clear"
                render_status_bar(
                    f,
                    area,
                    StatusCounts { matched: 5, filtered: 5, total: 10 },
                    0,
                    "type:user |",
                    None,
                    None,
                );
            })
            .unwrap();
    }

    #[test]
    fn test_render_status_bar_with_success_message() {
        use std::time::{Duration, Instant};

        let backend = TestBackend::new(100, 1);
        let mut terminal = Terminal::new(backend).unwrap();

        let status_msg = StatusMessage {
            text: "✓ Copied to clipboard".to_string(),
            message_type: MessageType::Success,
            expires_at: Instant::now() + Duration::from_secs(3),
        };

        terminal
            .draw(|f| {
                let area = f.area();
                render_status_bar(
                    f,
                    area,
                    StatusCounts { matched: 5, filtered: 5, total: 10 },
                    0,
                    "search",
                    None,
                    Some(&status_msg),
                );
            })
            .unwrap();

        // Verify rendering succeeded (color verification would require inspecting buffer)
    }

    #[test]
    fn test_render_status_bar_with_error_message() {
        use std::time::{Duration, Instant};

        let backend = TestBackend::new(100, 1);
        let mut terminal = Terminal::new(backend).unwrap();

        let status_msg = StatusMessage {
            text: "✗ No entries to copy".to_string(),
            message_type: MessageType::Error,
            expires_at: Instant::now() + Duration::from_secs(3),
        };

        terminal
            .draw(|f| {
                let area = f.area();
                render_status_bar(
                    f,
                    area,
                    StatusCounts { matched: 0, filtered: 0, total: 10 },
                    0,
                    "search",
                    None,
                    Some(&status_msg),
                );
            })
            .unwrap();

        // Verify rendering succeeded (color verification would require inspecting buffer)
    }

    #[test]
    fn test_render_status_bar_status_message_priority() {
        use std::time::{Duration, Instant};

        let backend = TestBackend::new(100, 1);
        let mut terminal = Terminal::new(backend).unwrap();

        // Both status message and filter error present
        let status_msg = StatusMessage {
            text: "✓ Copied to clipboard".to_string(),
            message_type: MessageType::Success,
            expires_at: Instant::now() + Duration::from_secs(3),
        };

        terminal
            .draw(|f| {
                let area = f.area();
                // Status message should take priority over filter error
                render_status_bar(
                    f,
                    area,
                    StatusCounts { matched: 5, filtered: 5, total: 10 },
                    0,
                    "search",
                    Some("This error should be hidden"),
                    Some(&status_msg),
                );
            })
            .unwrap();

        // Verify rendering succeeded (status message should be shown, not filter error)
    }

    #[test]
    fn test_render_ui_with_status_message() {
        use std::time::{Duration, Instant};

        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();

        let entries = [create_test_entry("First entry")];
        let entry_refs: Vec<&SearchEntry> = entries.iter().collect();

        let status_msg = StatusMessage {
            text: "✓ Copied to clipboard".to_string(),
            message_type: MessageType::Success,
            expires_at: Instant::now() + Duration::from_secs(3),
        };

        terminal
            .draw(|f| {
                let state = RenderState {
                    search_query: "test",
                    filtered_count: 1,
                    total_count: 1,
                    filter_error: None,
                    status_message: Some(&status_msg),
                };
                render_ui(f, &entry_refs, 0, &state);
            })
            .unwrap();
    }
}
