use std::env;
use std::path::PathBuf;

use anyhow::{Context, Result};

/// Get the Claude directory path (~/.claude)
pub fn get_claude_dir() -> Result<PathBuf> {
    let home = env::var("HOME").context("HOME environment variable not set")?;
    Ok(PathBuf::from(home).join(".claude"))
}

#[cfg(test)]
mod tests {
    use std::env;

    use super::*;

    #[test]
    fn test_get_claude_dir_with_valid_home() {
        // Save original HOME value
        let original_home = env::var("HOME").ok();

        // SAFETY: Setting environment variables in tests is safe as long as:
        // 1. Tests don't run in parallel accessing the same env var (we restore it)
        // 2. No other threads are reading this variable concurrently
        // 3. We restore the original value afterwards
        unsafe {
            env::set_var("HOME", "/Users/testuser");
        }

        let result = get_claude_dir();
        assert!(result.is_ok());
        let claude_dir = result.unwrap();
        assert_eq!(claude_dir, PathBuf::from("/Users/testuser/.claude"));

        // Restore original HOME
        if let Some(home) = original_home {
            unsafe {
                env::set_var("HOME", home);
            }
        }
    }

    #[test]
    fn test_get_claude_dir_missing_home() {
        // Save original HOME value
        let original_home = env::var("HOME").ok();

        // SAFETY: Removing environment variables in tests is safe as long as we restore it
        unsafe {
            env::remove_var("HOME");
        }

        let result = get_claude_dir();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("HOME environment variable not set"));

        // Restore original HOME
        if let Some(home) = original_home {
            unsafe {
                env::set_var("HOME", home);
            }
        }
    }
}
