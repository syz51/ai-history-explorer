use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectInfo {
    pub encoded_name: String,
    pub decoded_path: PathBuf,
    pub project_dir: PathBuf,
    pub agent_files: Vec<PathBuf>,
}
