use std::env;
use std::path::PathBuf;

use anyhow::{Context, Result};

/// Get the Claude directory path (~/.claude)
pub fn get_claude_dir() -> Result<PathBuf> {
    let home = env::var("HOME").context("HOME environment variable not set")?;
    Ok(PathBuf::from(home).join(".claude"))
}
