use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};

/// User actions from keyboard events
#[derive(Debug, PartialEq)]
pub enum Action {
    Quit,
    ClearSearch,
    MoveUp,
    MoveDown,
    PageUp,
    PageDown,
    ApplyFilter,
    CopyToClipboard,
    ToggleFilter,
    ToggleFocus,
    Refresh,
    UpdateSearch(char),
    DeleteChar,
    None,
}

/// Poll for keyboard events and convert to actions
pub fn poll_event(timeout: Duration) -> anyhow::Result<Action> {
    if event::poll(timeout)?
        && let Event::Key(key) = event::read()?
    {
        return Ok(key_to_action(key));
    }
    Ok(Action::None)
}

fn key_to_action(key: KeyEvent) -> Action {
    match (key.code, key.modifiers) {
        // Quit
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => Action::Quit,
        (KeyCode::Esc, _) => Action::ClearSearch,

        // Navigation (Vim/Emacs style)
        (KeyCode::Char('p'), KeyModifiers::CONTROL) => Action::MoveUp,
        (KeyCode::Char('n'), KeyModifiers::CONTROL) => Action::MoveDown,
        (KeyCode::Up, _) => Action::MoveUp,
        (KeyCode::Down, _) => Action::MoveDown,
        (KeyCode::PageUp, _) => Action::PageUp,
        (KeyCode::PageDown, _) => Action::PageDown,

        // Actions
        (KeyCode::Enter, _) => Action::ApplyFilter,
        (KeyCode::Char('y'), KeyModifiers::CONTROL) => Action::CopyToClipboard,
        (KeyCode::Char('/'), KeyModifiers::NONE) => Action::ToggleFilter,
        (KeyCode::Tab, _) => Action::ToggleFocus,
        (KeyCode::Char('r'), KeyModifiers::CONTROL) => Action::Refresh,

        // Search input
        (KeyCode::Char(c), KeyModifiers::NONE) | (KeyCode::Char(c), KeyModifiers::SHIFT) => {
            Action::UpdateSearch(c)
        }
        (KeyCode::Backspace, _) => Action::DeleteChar,

        _ => Action::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quit_actions() {
        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(key_to_action(ctrl_c), Action::Quit);
    }

    #[test]
    fn test_clear_search_action() {
        let esc = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        assert_eq!(key_to_action(esc), Action::ClearSearch);
    }

    #[test]
    fn test_navigation_vim_style() {
        let ctrl_p = KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL);
        assert_eq!(key_to_action(ctrl_p), Action::MoveUp);

        let ctrl_n = KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL);
        assert_eq!(key_to_action(ctrl_n), Action::MoveDown);
    }

    #[test]
    fn test_navigation_arrows() {
        let up = KeyEvent::new(KeyCode::Up, KeyModifiers::NONE);
        assert_eq!(key_to_action(up), Action::MoveUp);

        let down = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
        assert_eq!(key_to_action(down), Action::MoveDown);
    }

    #[test]
    fn test_search_input() {
        let char_a = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
        assert_eq!(key_to_action(char_a), Action::UpdateSearch('a'));

        let char_q = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        assert_eq!(key_to_action(char_q), Action::UpdateSearch('q'));

        let backspace = KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE);
        assert_eq!(key_to_action(backspace), Action::DeleteChar);
    }

    #[test]
    fn test_page_navigation() {
        let page_up = KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE);
        assert_eq!(key_to_action(page_up), Action::PageUp);

        let page_down = KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE);
        assert_eq!(key_to_action(page_down), Action::PageDown);
    }

    #[test]
    fn test_action_keys() {
        let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(key_to_action(enter), Action::ApplyFilter);

        let ctrl_y = KeyEvent::new(KeyCode::Char('y'), KeyModifiers::CONTROL);
        assert_eq!(key_to_action(ctrl_y), Action::CopyToClipboard);

        let slash = KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE);
        assert_eq!(key_to_action(slash), Action::ToggleFilter);

        let tab = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
        assert_eq!(key_to_action(tab), Action::ToggleFocus);

        let ctrl_r = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL);
        assert_eq!(key_to_action(ctrl_r), Action::Refresh);
    }

    #[test]
    fn test_search_input_with_shift() {
        let char_a_shift = KeyEvent::new(KeyCode::Char('A'), KeyModifiers::SHIFT);
        assert_eq!(key_to_action(char_a_shift), Action::UpdateSearch('A'));
    }

    #[test]
    fn test_unknown_key() {
        let unknown = KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE);
        assert_eq!(key_to_action(unknown), Action::None);
    }
}
