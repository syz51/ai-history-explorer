//! Cache persistence: load/save with atomic writes

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use bincode::config;

use super::metadata::{CACHE_VERSION, IndexMetadata};
use crate::models::SearchEntry;

const METADATA_FILENAME: &str = "index-metadata.json";
const INDEX_FILENAME: &str = "search-index.bin";

/// Compute hash of canonical path for cache subdirectory isolation
/// Returns first 12 characters of SHA256 hash
fn compute_path_hash(path: &Path) -> Result<String> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    // Canonicalize to handle symlinks and relative paths consistently
    let canonical = path.canonicalize().context("Failed to canonicalize path")?;

    // Use standard library hasher (fast, sufficient for cache isolation)
    let mut hasher = DefaultHasher::new();
    canonical.hash(&mut hasher);
    let hash = hasher.finish();

    // Convert to hex string, take first 12 chars
    Ok(format!("{:016x}", hash)[..12].to_string())
}

/// Get platform-specific cache directory for a specific Claude directory
pub fn get_cache_dir(claude_dir: &Path) -> Result<PathBuf> {
    let cache_base = dirs::cache_dir().context("Failed to get platform cache directory")?;

    // Compute hash for this specific Claude directory
    let path_hash = compute_path_hash(claude_dir)?;
    let cache_dir = cache_base.join("ai-history-explorer").join(path_hash);

    // Create directory if missing
    if !cache_dir.exists() {
        fs::create_dir_all(&cache_dir).context("Failed to create cache directory")?;
    }

    Ok(cache_dir)
}

/// Get path to metadata file
pub fn get_metadata_path(claude_dir: &Path) -> Result<PathBuf> {
    Ok(get_cache_dir(claude_dir)?.join(METADATA_FILENAME))
}

/// Get path to index file
pub fn get_index_path(claude_dir: &Path) -> Result<PathBuf> {
    Ok(get_cache_dir(claude_dir)?.join(INDEX_FILENAME))
}

/// Load cached index and metadata for a specific Claude directory
/// Returns None if cache is missing, corrupted, or version mismatch (caller should rebuild)
pub fn load_index(claude_dir: &Path) -> Result<Option<(Vec<SearchEntry>, IndexMetadata)>> {
    let metadata_path = get_metadata_path(claude_dir)?;
    let index_path = get_index_path(claude_dir)?;

    // Check if both files exist
    if !metadata_path.exists() || !index_path.exists() {
        return Ok(None);
    }

    // Load and parse metadata
    let metadata_json =
        fs::read_to_string(&metadata_path).context("Failed to read metadata file")?;
    let metadata: IndexMetadata =
        serde_json::from_str(&metadata_json).context("Failed to parse metadata JSON")?;

    // Check version compatibility
    if metadata.version != CACHE_VERSION {
        eprintln!(
            "Cache version mismatch (expected {}, found {}), rebuilding index",
            CACHE_VERSION, metadata.version
        );
        return Ok(None);
    }

    // Load and deserialize index
    let index_bytes = fs::read(&index_path).context("Failed to read index file")?;
    let entries: Vec<SearchEntry> =
        bincode::serde::decode_from_slice(&index_bytes, config::standard())
            .context("Failed to deserialize index")?
            .0;

    Ok(Some((entries, metadata)))
}

/// Save index and metadata atomically for a specific Claude directory
pub fn save_index(
    claude_dir: &Path,
    entries: &[SearchEntry],
    metadata: &IndexMetadata,
) -> Result<()> {
    let cache_dir = get_cache_dir(claude_dir)?;

    // Write metadata atomically (temp file + rename)
    let metadata_path = cache_dir.join(METADATA_FILENAME);
    let metadata_temp = cache_dir.join(format!("{}.tmp", METADATA_FILENAME));
    let metadata_json =
        serde_json::to_string_pretty(metadata).context("Failed to serialize metadata")?;
    fs::write(&metadata_temp, metadata_json).context("Failed to write metadata temp file")?;
    fs::rename(&metadata_temp, &metadata_path).context("Failed to rename metadata temp file")?;

    // Write index atomically (temp file + rename)
    let index_path = cache_dir.join(INDEX_FILENAME);
    let index_temp = cache_dir.join(format!("{}.tmp", INDEX_FILENAME));
    let index_bytes = bincode::serde::encode_to_vec(entries, config::standard())
        .context("Failed to serialize index")?;
    fs::write(&index_temp, index_bytes).context("Failed to write index temp file")?;
    fs::rename(&index_temp, &index_path).context("Failed to rename index temp file")?;

    Ok(())
}
