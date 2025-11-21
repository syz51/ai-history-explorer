use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::models::ProjectInfo;
use crate::utils::decode_and_validate_path;

/// Discover all projects in ~/.claude/projects/ and find agent-*.jsonl files
///
/// Scans the Claude projects directory for project subdirectories, decoding their
/// percent-encoded names back to file system paths and collecting all agent conversation
/// files within each project.
///
/// # Arguments
///
/// * `claude_dir` - Path to the ~/.claude directory
///
/// # Returns
///
/// Returns a Vec of [`ProjectInfo`] containing decoded paths and agent file locations.
/// Returns an empty Vec if the projects directory doesn't exist (not an error).
///
/// # Errors
///
/// Returns an error if:
/// - The projects directory exists but cannot be read
/// - A directory entry cannot be accessed
///
/// Individual project directories with invalid encoded names or read errors are logged
/// as warnings and skipped (graceful degradation).
pub fn discover_projects(claude_dir: &Path) -> Result<Vec<ProjectInfo>> {
    let projects_dir = claude_dir.join("projects");

    // Return empty vec if projects directory doesn't exist
    if !projects_dir.exists() {
        return Ok(Vec::new());
    }

    let mut projects = Vec::new();

    // Iterate through all entries in the projects directory
    let entries = fs::read_dir(&projects_dir)
        .context(format!("Failed to read projects directory: {}", projects_dir.display()))?;

    for entry in entries {
        let entry = entry.context("Failed to read directory entry")?;
        let path = entry.path();

        // Skip if not a directory
        if !path.is_dir() {
            continue;
        }

        // Get the directory name (encoded project path)
        let encoded_name = match path.file_name() {
            Some(name) => name.to_string_lossy().to_string(),
            None => continue,
        };

        // Decode and validate the project path
        let decoded_path = match decode_and_validate_path(&encoded_name) {
            Ok(path) => path,
            Err(e) => {
                eprintln!("Warning: Skipping invalid project directory {}: {}", encoded_name, e);
                continue;
            }
        };

        // Find all agent-*.jsonl files in this project directory
        let mut agent_files = Vec::new();
        match fs::read_dir(&path) {
            Ok(files) => {
                for file in files.flatten() {
                    let file_path = file.path();
                    if let Some(filename) = file_path.file_name() {
                        let filename_str = filename.to_string_lossy();
                        if filename_str.starts_with("agent-") && filename_str.ends_with(".jsonl") {
                            agent_files.push(file_path);
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Warning: Failed to read project directory {}: {}", path.display(), e);
                continue;
            }
        }

        projects.push(ProjectInfo { encoded_name, decoded_path, project_dir: path, agent_files });
    }

    Ok(projects)
}
