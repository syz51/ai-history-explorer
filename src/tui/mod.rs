// TUI module for interactive search interface
mod app;
mod events;
mod layout;
mod rendering;
mod terminal;
mod timestamps;

use anyhow::Result;
pub use app::App;
use terminal::TerminalManager;

use crate::models::SearchEntry;

/// Run the interactive TUI
pub fn run_interactive(entries: Vec<SearchEntry>) -> Result<()> {
    let mut manager = TerminalManager::new()?;
    let mut app = App::new(entries);

    let result = app.run(manager.terminal_mut());

    // Restore terminal (Drop will also clean up if this fails)
    manager.restore()?;

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_interactive_with_empty_entries() {
        // This would normally require a TTY, so we just test the app creation
        let entries = vec![];
        let app = App::new(entries);
        // Verify app was created successfully
        drop(app);
    }
}
