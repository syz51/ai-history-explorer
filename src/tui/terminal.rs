use std::io;

use anyhow::Result;
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

/// Manages terminal setup and cleanup
pub struct TerminalManager {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
}

impl TerminalManager {
    /// Set up terminal for TUI mode
    pub fn new() -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        Ok(Self { terminal })
    }

    /// Get mutable reference to terminal
    pub fn terminal_mut(&mut self) -> &mut Terminal<CrosstermBackend<io::Stdout>> {
        &mut self.terminal
    }

    /// Restore terminal to normal mode
    pub fn restore(mut self) -> Result<()> {
        disable_raw_mode()?;
        execute!(self.terminal.backend_mut(), LeaveAlternateScreen)?;
        self.terminal.show_cursor()?;
        Ok(())
    }
}

// Ensure cleanup happens even if dropped (panic, early return, etc.)
impl Drop for TerminalManager {
    fn drop(&mut self) {
        // Best effort cleanup - ignore errors since we're already unwinding
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_manager_drop_safety() {
        // Just verify that TerminalManager can be created and dropped
        // This tests the Drop implementation
        // Note: This will fail in CI without a TTY, so we just test the logic
        let result = TerminalManager::new();

        // If we have a terminal, verify it can be restored
        if let Ok(manager) = result {
            assert!(manager.restore().is_ok());
        }
    }
}
