use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Split-pane layout configuration
pub struct AppLayout {
    pub results_area: Rect,
    pub preview_area: Rect,
    pub status_area: Rect,
}

impl AppLayout {
    /// Create split-pane layout:
    /// - Results list: 60% width (left)
    /// - Preview pane: 40% width (right)
    /// - Status bar: bottom row
    pub fn new(area: Rect) -> Self {
        // Vertical split: main area + status bar
        let vertical_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(3),    // Main area (at least 3 rows)
                Constraint::Length(1), // Status bar (1 row)
            ])
            .split(area);

        // Horizontal split: results + preview
        let horizontal_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(60), // Results list
                Constraint::Percentage(40), // Preview pane
            ])
            .split(vertical_chunks[0]);

        Self {
            results_area: horizontal_chunks[0],
            preview_area: horizontal_chunks[1],
            status_area: vertical_chunks[1],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_splits_correctly() {
        let area = Rect::new(0, 0, 100, 30);
        let layout = AppLayout::new(area);

        // Status bar should be 1 row at bottom
        assert_eq!(layout.status_area.height, 1);
        assert_eq!(layout.status_area.y, 29);

        // Main area should be remaining rows
        assert_eq!(layout.results_area.height, 29);
        assert_eq!(layout.preview_area.height, 29);

        // Results should be ~60% width
        assert_eq!(layout.results_area.width, 60);

        // Preview should be ~40% width
        assert_eq!(layout.preview_area.width, 40);
    }

    #[test]
    fn test_layout_minimum_height() {
        let area = Rect::new(0, 0, 100, 4);
        let layout = AppLayout::new(area);

        // Status bar gets 1 row
        assert_eq!(layout.status_area.height, 1);
        // Main area gets remaining rows
        assert_eq!(layout.results_area.height, 3);
        assert_eq!(layout.preview_area.height, 3);
    }
}
