use std::borrow::Cow;
use std::env;
use std::fs::File;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use percent_encoding::{AsciiSet, CONTROLS, percent_decode_str, utf8_percent_encode};

// Maximum file size for JSONL files: 10MB
const MAX_FILE_SIZE_BYTES: u64 = 10 * 1024 * 1024;

// Define characters to percent-encode (everything except alphanumeric and safe chars)
const ENCODE_SET: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'<')
    .add(b'>')
    .add(b'`')
    .add(b'?')
    .add(b'{')
    .add(b'}')
    .add(b'/')
    .add(b':')
    .add(b'@')
    .add(b'[')
    .add(b']')
    .add(b'!');

/// Encodes a file system path into Claude's project directory format using percent encoding
///
/// # Examples
///
/// ```
/// use std::path::PathBuf;
/// use ai_history_explorer::encode_path;
///
/// let path = PathBuf::from("/Users/foo/bar");
/// assert_eq!(encode_path(&path), "-Users%2Ffoo%2Fbar");
/// ```
pub fn encode_path(path: &Path) -> String {
    let path_str = path.to_string_lossy();
    // Strip leading slash to avoid encoding it
    let without_leading_slash = path_str.strip_prefix('/').unwrap_or(&path_str);
    let encoded = utf8_percent_encode(without_leading_slash, ENCODE_SET).to_string();
    // Prepend hyphen to match Claude's format
    format!("-{}", encoded)
}

/// Decodes Claude's project directory format back to a file system path
///
/// # Examples
///
/// ```
/// use std::path::PathBuf;
/// use ai_history_explorer::decode_path;
///
/// let encoded = "-Users%2Ffoo%2Fbar";
/// assert_eq!(decode_path(encoded), PathBuf::from("/Users/foo/bar"));
/// ```
pub fn decode_path(encoded: &str) -> PathBuf {
    // Remove leading hyphen
    let without_prefix = encoded.strip_prefix('-').unwrap_or(encoded);

    // Percent-decode the string (avoiding double allocation)
    let decoded = percent_decode_str(without_prefix).decode_utf8_lossy();
    let decoded_str = match decoded {
        Cow::Borrowed(s) => s,
        Cow::Owned(ref s) => s.as_str(),
    };

    // Add back the leading slash for absolute paths
    PathBuf::from(format!("/{}", decoded_str))
}

/// Validates that a decoded path is safe and doesn't contain path traversal sequences
///
/// # Errors
///
/// Returns an error if:
/// - The path contains '..' components (path traversal)
/// - The path is not absolute
pub fn validate_decoded_path(path: &Path) -> Result<()> {
    // Check for path traversal via '..' components
    for component in path.components() {
        if component == std::path::Component::ParentDir {
            bail!("Path contains '..' component: {}", path.display());
        }
    }

    // Additional validation: ensure path is absolute
    if !path.is_absolute() {
        bail!("Path must be absolute: {}", path.display());
    }

    Ok(())
}

/// Decodes and validates a path in one operation
///
/// Convenience function that combines [`decode_path`] and [`validate_decoded_path`].
///
/// # Errors
///
/// Returns an error if the decoded path contains path traversal sequences or is not absolute.
pub fn decode_and_validate_path(encoded: &str) -> Result<PathBuf> {
    let decoded = decode_path(encoded);
    validate_decoded_path(&decoded)?;
    Ok(decoded)
}

/// Validates that a file's size is within acceptable limits (10MB)
///
/// Takes an open file handle to avoid TOCTOU (time-of-check-time-of-use)
/// race conditions where the file could be modified between the size check
/// and subsequent file operations.
///
/// # Errors
///
/// Returns an error if:
/// - The file metadata cannot be read
/// - The file is larger than 10MB
pub fn validate_file_size(file: &File, path: &Path) -> Result<()> {
    let metadata = file
        .metadata()
        .with_context(|| format!("Failed to read file metadata: {}", path.display()))?;

    let file_size = metadata.len();
    if file_size > MAX_FILE_SIZE_BYTES {
        bail!(
            "File too large: {} ({} bytes, max {} bytes)",
            path.display(),
            file_size,
            MAX_FILE_SIZE_BYTES
        );
    }

    Ok(())
}

/// Formats a path with ~ substitution for the home directory
///
/// # Examples
///
/// ```no_run
/// use std::path::PathBuf;
/// use ai_history_explorer::format_path_with_tilde;
///
/// let path = PathBuf::from("/Users/alice/Documents");
/// // Returns "~/Documents" if HOME=/Users/alice
/// let formatted = format_path_with_tilde(&path);
/// ```
pub fn format_path_with_tilde(path: &Path) -> String {
    format_path_with_tilde_internal(path, None)
}

/// Internal helper for path formatting with optional home override (for testing)
pub(crate) fn format_path_with_tilde_internal(path: &Path, home_override: Option<&str>) -> String {
    let home_from_env = env::var("HOME").ok();
    let home = home_override.or(home_from_env.as_deref());

    let path_str = path.to_string_lossy();
    if let Some(home) = home
        && path_str.starts_with(home)
    {
        return path_str.replacen(home, "~", 1);
    }

    // Avoid double allocation when converting Cow to String
    match path_str {
        Cow::Borrowed(s) => s.to_string(),
        Cow::Owned(s) => s,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_path() {
        let path = PathBuf::from("/Users/foo/bar");
        assert_eq!(encode_path(&path), "-Users%2Ffoo%2Fbar");
    }

    #[test]
    fn test_decode_path() {
        let encoded = "-Users%2Ffoo%2Fbar";
        let expected = PathBuf::from("/Users/foo/bar");
        assert_eq!(decode_path(encoded), expected);
    }

    #[test]
    fn test_no_collision() {
        // These two different paths should encode differently
        let path1 = PathBuf::from("/foo/bar");
        let path2 = PathBuf::from("/foo-bar");
        assert_ne!(encode_path(&path1), encode_path(&path2));
    }

    #[test]
    fn test_validate_safe_path() {
        let safe_path = PathBuf::from("/Users/foo/bar");
        assert!(validate_decoded_path(&safe_path).is_ok());
    }

    #[test]
    fn test_validate_path_with_parent_dir() {
        let unsafe_path = PathBuf::from("/Users/foo/../etc/passwd");
        assert!(validate_decoded_path(&unsafe_path).is_err());
    }

    #[test]
    fn test_validate_relative_path() {
        let relative = PathBuf::from("Users/foo/bar");
        assert!(validate_decoded_path(&relative).is_err());
    }

    #[test]
    fn test_decode_and_validate_safe() {
        let encoded = "-Users%2Ffoo%2Fbar";
        assert!(decode_and_validate_path(encoded).is_ok());
    }

    #[test]
    fn test_decode_and_validate_traversal() {
        // This encoded string decodes to /Users/foo/../etc/passwd
        let encoded = "-Users%2Ffoo%2F..%2Fetc%2Fpasswd";
        assert!(decode_and_validate_path(encoded).is_err());
    }

    #[test]
    fn test_roundtrip() {
        let original = PathBuf::from("/Users/test/Documents/project");
        let encoded = encode_path(&original);
        let decoded = decode_path(&encoded);
        assert_eq!(original, decoded);
    }

    #[test]
    fn test_format_path_with_tilde() {
        // Test with explicit home directory (no unsafe needed)
        let path = PathBuf::from("/Users/testuser/Documents/project");
        let formatted = format_path_with_tilde_internal(&path, Some("/Users/testuser"));
        assert_eq!(formatted, "~/Documents/project");

        // Path not under home
        let path2 = PathBuf::from("/opt/local/bin");
        let formatted2 = format_path_with_tilde_internal(&path2, Some("/Users/testuser"));
        assert_eq!(formatted2, "/opt/local/bin");

        // Test with None (uses actual env var, but won't fail if not set)
        let path3 = PathBuf::from("/some/random/path");
        let formatted3 = format_path_with_tilde_internal(&path3, None);
        // Just verify it doesn't crash
        assert!(!formatted3.is_empty());
    }
}
