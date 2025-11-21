use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::models::ProjectInfo;
use crate::utils::{decode_and_validate_path, validate_path_not_symlink};

/// Maximum number of projects to process (security: prevent resource exhaustion)
const MAX_PROJECTS: usize = 1000;

/// Maximum number of agent files per project (security: prevent resource exhaustion)
const MAX_AGENT_FILES_PER_PROJECT: usize = 1000;

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
/// - More than [`MAX_PROJECTS`] (1000) projects are found (security: resource exhaustion)
/// - A project has more than [`MAX_AGENT_FILES_PER_PROJECT`] (1000) agent files
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

        // Security: Validate project directory is not a symlink
        if let Err(e) = validate_path_not_symlink(&path) {
            eprintln!(
                "Warning: Skipping project directory (symlink not allowed) {}: {}",
                encoded_name, e
            );
            continue;
        }

        // Security: Enforce maximum projects limit
        if projects.len() >= MAX_PROJECTS {
            bail!(
                "Resource limit exceeded: Found more than {} projects. This may indicate a misconfiguration or attack.",
                MAX_PROJECTS
            );
        }

        // Find all agent-*.jsonl files in this project directory
        let mut agent_files = Vec::new();
        match fs::read_dir(&path) {
            Ok(files) => {
                for file in files.flatten() {
                    let file_path = file.path();
                    if let Some(filename) = file_path.file_name() {
                        let filename_str = filename.to_string_lossy();
                        if filename_str.starts_with("agent-") && filename_str.ends_with(".jsonl") {
                            // Security: Enforce maximum agent files per project limit
                            if agent_files.len() >= MAX_AGENT_FILES_PER_PROJECT {
                                bail!(
                                    "Resource limit exceeded: Project {} has more than {} agent files",
                                    encoded_name,
                                    MAX_AGENT_FILES_PER_PROJECT
                                );
                            }

                            // Security: Skip symlinked agent files
                            if let Err(e) = validate_path_not_symlink(&file_path) {
                                eprintln!(
                                    "Warning: Skipping agent file (symlink not allowed) {}: {}",
                                    file_path.display(),
                                    e
                                );
                                continue;
                            }
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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;
    use std::path::{Path, PathBuf};

    use tempfile::TempDir;

    use super::*;

    /// Helper to create a test .claude directory structure
    fn create_test_claude_dir() -> TempDir {
        TempDir::new().expect("Failed to create temp dir")
    }

    /// Helper to create a project directory with optional agent files
    fn create_project_dir(
        projects_dir: &Path,
        encoded_name: &str,
        agent_files: &[&str],
    ) -> PathBuf {
        let project_dir = projects_dir.join(encoded_name);
        fs::create_dir(&project_dir).expect("Failed to create project dir");

        for filename in agent_files {
            let file_path = project_dir.join(filename);
            let mut file = fs::File::create(file_path).expect("Failed to create agent file");
            file.write_all(b"test content").expect("Failed to write agent file");
        }

        project_dir
    }

    #[test]
    fn test_discover_projects_with_valid_structure() {
        let claude_dir = create_test_claude_dir();
        let projects_dir = claude_dir.path().join("projects");
        fs::create_dir(&projects_dir).expect("Failed to create projects dir");

        // Create projects with agent files
        create_project_dir(&projects_dir, "-Users%2Ftest%2Fproject1", &["agent-123.jsonl"]);
        create_project_dir(&projects_dir, "-Users%2Ftest%2Fproject2", &["agent-456.jsonl"]);

        let result = discover_projects(claude_dir.path());
        assert!(result.is_ok());
        let mut projects = result.unwrap();

        assert_eq!(projects.len(), 2);

        // Sort by encoded_name for consistent ordering
        projects.sort_by(|a, b| a.encoded_name.cmp(&b.encoded_name));

        // Check first project
        assert_eq!(projects[0].encoded_name, "-Users%2Ftest%2Fproject1");
        assert_eq!(projects[0].decoded_path, PathBuf::from("/Users/test/project1"));
        assert_eq!(projects[0].agent_files.len(), 1);
        assert!(projects[0].agent_files[0].ends_with("agent-123.jsonl"));

        // Check second project
        assert_eq!(projects[1].encoded_name, "-Users%2Ftest%2Fproject2");
        assert_eq!(projects[1].decoded_path, PathBuf::from("/Users/test/project2"));
        assert_eq!(projects[1].agent_files.len(), 1);
        assert!(projects[1].agent_files[0].ends_with("agent-456.jsonl"));
    }

    #[test]
    fn test_discover_projects_missing_directory() {
        let claude_dir = create_test_claude_dir();

        // Don't create projects directory
        let result = discover_projects(claude_dir.path());
        assert!(result.is_ok());
        let projects = result.unwrap();

        // Should return empty vec, not error
        assert_eq!(projects.len(), 0);
    }

    #[test]
    fn test_discover_projects_with_multiple_agent_files() {
        let claude_dir = create_test_claude_dir();
        let projects_dir = claude_dir.path().join("projects");
        fs::create_dir(&projects_dir).expect("Failed to create projects dir");

        // Create project with multiple agent files
        create_project_dir(
            &projects_dir,
            "-Users%2Ftest%2Fproject",
            &["agent-123.jsonl", "agent-456.jsonl", "agent-789.jsonl"],
        );

        let result = discover_projects(claude_dir.path());
        assert!(result.is_ok());
        let projects = result.unwrap();

        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].agent_files.len(), 3);

        // Verify all agent files are found
        let filenames: Vec<String> = projects[0]
            .agent_files
            .iter()
            .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
            .collect();
        assert!(filenames.contains(&"agent-123.jsonl".to_string()));
        assert!(filenames.contains(&"agent-456.jsonl".to_string()));
        assert!(filenames.contains(&"agent-789.jsonl".to_string()));
    }

    #[test]
    fn test_discover_projects_skips_non_agent_files() {
        let claude_dir = create_test_claude_dir();
        let projects_dir = claude_dir.path().join("projects");
        fs::create_dir(&projects_dir).expect("Failed to create projects dir");

        // Create project with agent and non-agent files
        let project_dir = projects_dir.join("-Users%2Ftest%2Fproject");
        fs::create_dir(&project_dir).expect("Failed to create project dir");

        // Create various files
        fs::File::create(project_dir.join("agent-123.jsonl")).expect("Failed to create file");
        fs::File::create(project_dir.join("history.jsonl")).expect("Failed to create file");
        fs::File::create(project_dir.join("readme.txt")).expect("Failed to create file");
        fs::File::create(project_dir.join("other-file.jsonl")).expect("Failed to create file");

        let result = discover_projects(claude_dir.path());
        assert!(result.is_ok());
        let projects = result.unwrap();

        assert_eq!(projects.len(), 1);
        // Should only include agent-*.jsonl files
        assert_eq!(projects[0].agent_files.len(), 1);
        assert!(projects[0].agent_files[0].ends_with("agent-123.jsonl"));
    }

    #[test]
    fn test_discover_projects_skips_non_directories() {
        let claude_dir = create_test_claude_dir();
        let projects_dir = claude_dir.path().join("projects");
        fs::create_dir(&projects_dir).expect("Failed to create projects dir");

        // Create a regular file in projects directory
        fs::File::create(projects_dir.join("not-a-directory.txt")).expect("Failed to create file");

        // Create a valid project
        create_project_dir(&projects_dir, "-Users%2Ftest%2Fproject", &["agent-123.jsonl"]);

        let result = discover_projects(claude_dir.path());
        assert!(result.is_ok());
        let projects = result.unwrap();

        // Should only find the valid project directory
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].encoded_name, "-Users%2Ftest%2Fproject");
    }

    #[test]
    fn test_discover_projects_invalid_encoded_name() {
        let claude_dir = create_test_claude_dir();
        let projects_dir = claude_dir.path().join("projects");
        fs::create_dir(&projects_dir).expect("Failed to create projects dir");

        // Create project with path traversal in encoded name
        create_project_dir(&projects_dir, "-Users%2F..%2Fetc%2Fpasswd", &["agent-123.jsonl"]);

        // Create a valid project too
        create_project_dir(&projects_dir, "-Users%2Ftest%2Fproject", &["agent-456.jsonl"]);

        let result = discover_projects(claude_dir.path());
        assert!(result.is_ok());
        let projects = result.unwrap();

        // Should skip invalid project and only return valid one
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].encoded_name, "-Users%2Ftest%2Fproject");
    }

    #[test]
    fn test_discover_projects_no_agent_files() {
        let claude_dir = create_test_claude_dir();
        let projects_dir = claude_dir.path().join("projects");
        fs::create_dir(&projects_dir).expect("Failed to create projects dir");

        // Create project without agent files
        create_project_dir(&projects_dir, "-Users%2Ftest%2Fproject", &[]);

        let result = discover_projects(claude_dir.path());
        assert!(result.is_ok());
        let projects = result.unwrap();

        // Should still include project but with empty agent_files
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].agent_files.len(), 0);
    }

    #[test]
    fn test_discover_projects_empty_projects_directory() {
        let claude_dir = create_test_claude_dir();
        let projects_dir = claude_dir.path().join("projects");
        fs::create_dir(&projects_dir).expect("Failed to create projects dir");

        // Empty directory
        let result = discover_projects(claude_dir.path());
        assert!(result.is_ok());
        let projects = result.unwrap();

        assert_eq!(projects.len(), 0);
    }

    #[test]
    fn test_discover_projects_preserves_project_dir_path() {
        let claude_dir = create_test_claude_dir();
        let projects_dir = claude_dir.path().join("projects");
        fs::create_dir(&projects_dir).expect("Failed to create projects dir");

        create_project_dir(&projects_dir, "-Users%2Ftest%2Fproject", &["agent-123.jsonl"]);

        let result = discover_projects(claude_dir.path());
        assert!(result.is_ok());
        let projects = result.unwrap();

        assert_eq!(projects.len(), 1);

        // Verify project_dir is the actual directory in .claude/projects/
        assert_eq!(projects[0].project_dir, projects_dir.join("-Users%2Ftest%2Fproject"));
    }

    #[test]
    fn test_discover_projects_handles_special_characters() {
        let claude_dir = create_test_claude_dir();
        let projects_dir = claude_dir.path().join("projects");
        fs::create_dir(&projects_dir).expect("Failed to create projects dir");

        // Create project with special characters in path
        create_project_dir(
            &projects_dir,
            "-Users%2Ftest%2Fmy%20project%20%28v1%29",
            &["agent-123.jsonl"],
        );

        let result = discover_projects(claude_dir.path());
        assert!(result.is_ok());
        let projects = result.unwrap();

        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].decoded_path, PathBuf::from("/Users/test/my project (v1)"));
    }

    // ===== Security Tests: Resource Limits =====

    #[test]
    fn test_discover_projects_max_projects_limit() {
        let claude_dir = create_test_claude_dir();
        let projects_dir = claude_dir.path().join("projects");
        fs::create_dir(&projects_dir).expect("Failed to create projects dir");

        // Create MAX_PROJECTS + 1 projects (should fail)
        for i in 0..=MAX_PROJECTS {
            create_project_dir(
                &projects_dir,
                &format!("-Users%2Ftest%2Fproject{}", i),
                &["agent-1.jsonl"],
            );
        }

        let result = discover_projects(claude_dir.path());
        assert!(result.is_err(), "Should fail when exceeding max projects");
        assert!(
            result.unwrap_err().to_string().contains("Resource limit exceeded"),
            "Error should mention resource limit"
        );
    }

    #[test]
    fn test_discover_projects_exactly_max_projects() {
        let claude_dir = create_test_claude_dir();
        let projects_dir = claude_dir.path().join("projects");
        fs::create_dir(&projects_dir).expect("Failed to create projects dir");

        // Create exactly MAX_PROJECTS (should succeed)
        for i in 0..MAX_PROJECTS {
            create_project_dir(
                &projects_dir,
                &format!("-Users%2Ftest%2Fproject{}", i),
                &["agent-1.jsonl"],
            );
        }

        let result = discover_projects(claude_dir.path());
        assert!(result.is_ok(), "Should succeed with exactly max projects");
        let projects = result.unwrap();
        assert_eq!(projects.len(), MAX_PROJECTS);
    }

    #[test]
    fn test_discover_projects_max_agent_files_limit() {
        let claude_dir = create_test_claude_dir();
        let projects_dir = claude_dir.path().join("projects");
        fs::create_dir(&projects_dir).expect("Failed to create projects dir");

        // Create project with MAX_AGENT_FILES_PER_PROJECT + 1 agent files (should fail)
        let agent_files: Vec<String> =
            (0..=MAX_AGENT_FILES_PER_PROJECT).map(|i| format!("agent-{}.jsonl", i)).collect();
        let agent_file_refs: Vec<&str> = agent_files.iter().map(|s| s.as_str()).collect();

        create_project_dir(&projects_dir, "-Users%2Ftest%2Fproject", &agent_file_refs);

        let result = discover_projects(claude_dir.path());
        assert!(result.is_err(), "Should fail when exceeding max agent files");
        assert!(
            result.unwrap_err().to_string().contains("Resource limit exceeded"),
            "Error should mention resource limit"
        );
    }

    #[test]
    fn test_discover_projects_exactly_max_agent_files() {
        let claude_dir = create_test_claude_dir();
        let projects_dir = claude_dir.path().join("projects");
        fs::create_dir(&projects_dir).expect("Failed to create projects dir");

        // Create project with exactly MAX_AGENT_FILES_PER_PROJECT (should succeed)
        let agent_files: Vec<String> =
            (0..MAX_AGENT_FILES_PER_PROJECT).map(|i| format!("agent-{}.jsonl", i)).collect();
        let agent_file_refs: Vec<&str> = agent_files.iter().map(|s| s.as_str()).collect();

        create_project_dir(&projects_dir, "-Users%2Ftest%2Fproject", &agent_file_refs);

        let result = discover_projects(claude_dir.path());
        assert!(result.is_ok(), "Should succeed with exactly max agent files");
        let projects = result.unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].agent_files.len(), MAX_AGENT_FILES_PER_PROJECT);
    }
}
