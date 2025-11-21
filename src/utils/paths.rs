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
    .add(b'!')
    .add(b'%');

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
/// This performs logical validation on the path structure without filesystem access.
///
/// # Errors
///
/// Returns an error if:
/// - The path contains '..' components (path traversal)
/// - The path is not absolute
///
/// # Security Note
///
/// This function only validates the path structure. For filesystem operations,
/// additionally call [`validate_path_not_symlink`] to prevent symlink-based attacks.
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

/// Validates that a filesystem path is not a symbolic link
///
/// # Security
///
/// Prevents symlink-based attacks where an attacker creates a symlink in the
/// `.claude/projects/` directory pointing to sensitive locations like `/etc/passwd`.
/// This must be called for paths that will be accessed on the filesystem.
///
/// # Errors
///
/// Returns an error if:
/// - The path does not exist
/// - The path metadata cannot be read
/// - The path is a symbolic link
pub fn validate_path_not_symlink(path: &Path) -> Result<()> {
    // Use symlink_metadata to get metadata without following symlinks
    let metadata = std::fs::symlink_metadata(path)
        .with_context(|| format!("Failed to read metadata for path: {}", path.display()))?;

    if metadata.is_symlink() {
        bail!("Path is a symbolic link (symlinks not allowed for security): {}", path.display());
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

/// Validates that a file is not a hardlink with multiple references
///
/// # Security
///
/// Prevents hardlink-based attacks where an attacker creates a hardlink from
/// within `.claude/` to a sensitive file outside the directory. On Unix systems,
/// this checks if the file has multiple hardlinks (nlink > 1).
///
/// # Errors
///
/// Returns an error if the file has multiple hardlinks on Unix systems.
#[cfg(unix)]
pub fn validate_not_hardlink(path: &Path) -> Result<()> {
    use std::os::unix::fs::MetadataExt;

    let metadata = std::fs::metadata(path)
        .with_context(|| format!("Failed to read metadata for: {}", path.display()))?;

    let nlink = metadata.nlink();
    if nlink > 1 {
        bail!("{} has {} hard links (possible hardlink attack)", path.display(), nlink);
    }

    Ok(())
}

#[cfg(not(unix))]
pub fn validate_not_hardlink(_path: &Path) -> Result<()> {
    // Windows: hardlinks less common, skip check
    Ok(())
}

/// Safely opens a file for reading with TOCTOU protection
///
/// # Security
///
/// This function prevents TOCTOU (Time-of-Check-Time-of-Use) race conditions by:
/// 1. Opening the file atomically with O_NOFOLLOW (won't follow symlinks)
/// 2. Validating size on the already-open file descriptor
/// 3. Checking it's a regular file (not FIFO, device, socket, etc.)
/// 4. Checking for hardlinks (multiple references to same inode)
///
/// # Errors
///
/// Returns an error if:
/// - The path is a symbolic link
/// - The file is larger than 10MB
/// - The file is not a regular file
/// - The file has multiple hardlinks (Unix only)
pub fn safe_open_file(path: &Path) -> Result<File> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, OpenOptionsExt};

        // Open with O_NOFOLLOW - fails atomically if path is symlink
        let file = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)
            .with_context(|| format!("Failed to open {}", path.display()))?;

        // Now validate size on the ALREADY OPEN file (same file descriptor)
        let metadata = file
            .metadata()
            .with_context(|| format!("Failed to read file metadata: {}", path.display()))?;

        let size = metadata.len();
        if size > MAX_FILE_SIZE_BYTES {
            bail!(
                "File too large: {} ({} bytes, max {} bytes)",
                path.display(),
                size,
                MAX_FILE_SIZE_BYTES
            );
        }

        // Check it's a regular file (not FIFO, device, etc.)
        let mode = metadata.mode();
        if mode & (libc::S_IFMT as u32) != (libc::S_IFREG as u32) {
            bail!("{} is not a regular file", path.display());
        }

        // Check for hardlinks
        let nlink = metadata.nlink();
        if nlink > 1 {
            bail!("{} has {} hard links (possible hardlink attack)", path.display(), nlink);
        }

        Ok(file)
    }

    #[cfg(not(unix))]
    {
        // Windows: less comprehensive but still safe
        let file =
            File::open(path).with_context(|| format!("Failed to open {}", path.display()))?;

        validate_file_size(&file, path)?;

        let metadata = file
            .metadata()
            .with_context(|| format!("Failed to read file metadata: {}", path.display()))?;

        if !metadata.is_file() {
            bail!("{} is not a regular file", path.display());
        }

        Ok(file)
    }
}

/// Safely opens a directory for reading with TOCTOU protection
///
/// # Security
///
/// This function prevents symlink-based directory attacks by verifying
/// the path is not a symlink before opening.
///
/// # Errors
///
/// Returns an error if:
/// - The path is a symbolic link
/// - The path is not a directory
/// - The directory cannot be read
pub fn safe_open_dir(path: &Path) -> Result<std::fs::ReadDir> {
    // Get metadata without following symlinks
    let metadata = std::fs::symlink_metadata(path)
        .with_context(|| format!("Failed to read metadata for: {}", path.display()))?;

    if metadata.is_symlink() {
        bail!("{} is a symbolic link", path.display());
    }

    if !metadata.is_dir() {
        bail!("{} is not a directory", path.display());
    }

    // Open directory - on Unix, directories can't be symlinks
    // if we've already verified with symlink_metadata
    std::fs::read_dir(path).with_context(|| format!("Failed to read directory {}", path.display()))
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

    // ===== Security Tests: Path Traversal Attacks =====

    #[test]
    fn test_encoded_parent_dir_basic() {
        // Attack: -..%2Fetc%2Fpasswd -> /../etc/passwd
        let encoded = "-..%2Fetc%2Fpasswd";
        let result = decode_and_validate_path(encoded);
        assert!(result.is_err(), "Should reject encoded parent dir at start");
    }

    #[test]
    fn test_encoded_parent_dir_middle() {
        // Attack: -Users%2F..%2F..%2Fetc%2Fpasswd -> /Users/../../etc/passwd
        let encoded = "-Users%2F..%2F..%2Fetc%2Fpasswd";
        let result = decode_and_validate_path(encoded);
        assert!(result.is_err(), "Should reject encoded parent dir in middle");
    }

    #[test]
    fn test_double_encoded_parent_dir() {
        // Attack: double encoding %2E%2E%2F -> .. when decoded once, then ../ when decoded twice
        // Note: Our percent_decode_str only decodes once, so %2E%2E%2F becomes literally ".."
        let encoded = "-%2E%2E%2Fetc%2Fpasswd";
        let decoded = decode_path(encoded);
        // After single decode, %2E%2E becomes ".." literally
        let result = validate_decoded_path(&decoded);
        assert!(result.is_err(), "Should reject double-encoded parent dir");
    }

    #[test]
    fn test_mixed_encoding_parent_dir() {
        // Attack: -..%2Fsome%2Fpath (mixed literal .. with encoded /)
        let encoded = "-..%2Fsome%2Fpath";
        let result = decode_and_validate_path(encoded);
        assert!(result.is_err(), "Should reject mixed encoded parent dir");
    }

    #[test]
    fn test_absolute_sensitive_paths() {
        // Attack: legitimate encoding but pointing to sensitive files
        // While technically valid encoding, the validation should pass but
        // the application should handle sensitive paths carefully
        let etc_passwd = PathBuf::from("/etc/passwd");
        let encoded = encode_path(&etc_passwd);
        let result = decode_and_validate_path(&encoded);
        // This SHOULD pass validation (it's a valid absolute path without ..)
        // but the application must handle such paths carefully
        assert!(result.is_ok(), "Valid absolute path should pass validation");
        assert_eq!(result.unwrap(), PathBuf::from("/etc/passwd"));
    }

    #[test]
    fn test_path_with_existing_percent() {
        // Edge case: path that literally contains % character
        // % is now in ENCODE_SET, so it gets encoded as %25
        let path = PathBuf::from("/Users/foo%20bar/test");
        let encoded = encode_path(&path);
        // % should be encoded as %25, so %20 becomes %2520
        assert!(encoded.contains("%2520"), "Percent sign should be encoded as %25");
        assert!(!encoded.contains("%20"), "Original %20 should be double-encoded");
        let decoded = decode_path(&encoded);
        // Roundtrip should preserve the original path with %20
        assert_eq!(decoded, path, "Should preserve literal percent signs in roundtrip");
    }

    #[test]
    fn test_percent_encoding_prevents_double_decode_attack() {
        // Security test: prevent double-decode path traversal attack
        // If a directory name contains "%2E%2E" (percent-encoded ".."),
        // we must encode the % to prevent it from being decoded into ".."
        let malicious_path = PathBuf::from("/tmp/%2E%2E%2Fetc");
        let encoded = encode_path(&malicious_path);

        // The % should be encoded as %25, so %2E becomes %252E
        assert!(encoded.contains("%252E"), "Percent in %2E should be encoded");

        let decoded = decode_path(&encoded);
        // After roundtrip, should still contain literal %2E%2E, NOT ".."
        assert_eq!(decoded, PathBuf::from("/tmp/%2E%2E%2Fetc"));

        // Validation should pass (no literal ".." components)
        assert!(
            validate_decoded_path(&decoded).is_ok(),
            "Encoded percent sequences should not create path traversal"
        );
    }

    #[test]
    fn test_unicode_in_paths() {
        // Unicode characters in paths
        let path = PathBuf::from("/Users/测试/项目");
        let encoded = encode_path(&path);
        let decoded = decode_path(&encoded);
        assert_eq!(path, decoded, "Should handle Unicode paths");
        assert!(validate_decoded_path(&decoded).is_ok());
    }

    #[test]
    fn test_very_long_path() {
        // Test with a very long path (but not exceeding PATH_MAX)
        let long_component = "a".repeat(255); // Max filename length on most filesystems
        let path = PathBuf::from(format!(
            "/Users/{}/{}/{}",
            long_component, long_component, long_component
        ));
        let encoded = encode_path(&path);
        let decoded = decode_path(&encoded);
        assert_eq!(path, decoded, "Should handle long paths");
        assert!(validate_decoded_path(&decoded).is_ok());
    }

    #[test]
    fn test_empty_path_component() {
        // Path with empty component (/Users//foo -> /Users/foo after normalization)
        let encoded = "-Users%2F%2Ffoo"; // /Users//foo
        let decoded = decode_path(encoded);
        // Empty components are technically valid in Unix paths
        assert!(validate_decoded_path(&decoded).is_ok());
    }

    #[test]
    fn test_dot_single_component() {
        // Single dot (.) is allowed - it represents current directory
        let path = PathBuf::from("/Users/./foo");
        assert!(validate_decoded_path(&path).is_ok(), "Single dot should be allowed");
    }

    #[test]
    fn test_null_byte_in_path() {
        // Rust's Path/PathBuf handles null bytes, but they're invalid in actual filesystem paths
        // The encoding should preserve them, but filesystem operations will fail naturally
        let path_with_null = "/Users/foo\0bar";
        let encoded =
            utf8_percent_encode(path_with_null.strip_prefix('/').unwrap(), ENCODE_SET).to_string();
        let formatted = format!("-{}", encoded);
        let decoded = decode_path(&formatted);
        // Validation only checks for .. and absolute path, so this passes
        // But actual filesystem operations will fail
        assert!(validate_decoded_path(&decoded).is_ok());
    }

    #[test]
    fn test_special_chars_encoding() {
        // Test various special characters get encoded properly
        let path = PathBuf::from("/Users/foo bar/test@123");
        let encoded = encode_path(&path);
        // Space should be encoded
        assert!(encoded.contains("%20"), "Space should be percent-encoded");
        // @ should be encoded
        assert!(encoded.contains("%40"), "@ should be percent-encoded");
        let decoded = decode_path(&encoded);
        assert_eq!(path, decoded);
    }

    // ===== Security Tests: File Size Validation =====

    #[test]
    fn test_file_size_empty() {
        use std::io::Write;

        use tempfile::NamedTempFile;

        let mut temp = NamedTempFile::new().unwrap();
        // Empty file (0 bytes)
        temp.flush().unwrap();

        let file = File::open(temp.path()).unwrap();
        let result = validate_file_size(&file, temp.path());
        assert!(result.is_ok(), "Empty file should pass validation");
    }

    #[test]
    fn test_file_size_under_limit() {
        use std::io::Write;

        use tempfile::NamedTempFile;

        let mut temp = NamedTempFile::new().unwrap();
        // 1KB file
        temp.write_all(&vec![b'a'; 1024]).unwrap();
        temp.flush().unwrap();

        let file = File::open(temp.path()).unwrap();
        let result = validate_file_size(&file, temp.path());
        assert!(result.is_ok(), "Small file should pass validation");
    }

    #[test]
    fn test_file_size_exactly_10mb() {
        use std::io::Write;

        use tempfile::NamedTempFile;

        let mut temp = NamedTempFile::new().unwrap();
        // Exactly 10MB
        let chunk_size = 1024 * 1024; // 1MB chunks
        for _ in 0..10 {
            temp.write_all(&vec![b'a'; chunk_size]).unwrap();
        }
        temp.flush().unwrap();

        let file = File::open(temp.path()).unwrap();
        let result = validate_file_size(&file, temp.path());
        assert!(result.is_ok(), "Exactly 10MB file should pass validation");
    }

    #[test]
    fn test_file_size_10mb_plus_one() {
        use std::io::Write;

        use tempfile::NamedTempFile;

        let mut temp = NamedTempFile::new().unwrap();
        // 10MB + 1 byte
        let chunk_size = 1024 * 1024; // 1MB chunks
        for _ in 0..10 {
            temp.write_all(&vec![b'a'; chunk_size]).unwrap();
        }
        temp.write_all(b"x").unwrap(); // One extra byte
        temp.flush().unwrap();

        let file = File::open(temp.path()).unwrap();
        let result = validate_file_size(&file, temp.path());
        assert!(result.is_err(), "File over 10MB should fail validation");
        assert!(result.unwrap_err().to_string().contains("File too large"));
    }

    #[test]
    fn test_file_size_way_over_limit() {
        use std::io::Write;

        use tempfile::NamedTempFile;

        let mut temp = NamedTempFile::new().unwrap();
        // 20MB (way over limit)
        let chunk_size = 1024 * 1024; // 1MB chunks
        for _ in 0..20 {
            temp.write_all(&vec![b'a'; chunk_size]).unwrap();
        }
        temp.flush().unwrap();

        let file = File::open(temp.path()).unwrap();
        let result = validate_file_size(&file, temp.path());
        assert!(result.is_err(), "File way over limit should fail validation");
    }

    // ===== Security Tests: Symlink Validation =====

    #[test]
    fn test_validate_regular_file_not_symlink() {
        use tempfile::NamedTempFile;

        let temp = NamedTempFile::new().unwrap();
        let result = validate_path_not_symlink(temp.path());
        assert!(result.is_ok(), "Regular file should pass symlink validation");
    }

    #[test]
    fn test_validate_directory_not_symlink() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let result = validate_path_not_symlink(temp_dir.path());
        assert!(result.is_ok(), "Regular directory should pass symlink validation");
    }

    #[test]
    #[cfg(unix)] // Symlinks work differently on Windows
    fn test_validate_symlink_rejected() {
        use std::os::unix::fs::symlink;

        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let target = temp_dir.path().join("target");
        let link = temp_dir.path().join("link");

        // Create target directory
        std::fs::create_dir(&target).unwrap();

        // Create symlink pointing to target
        symlink(&target, &link).unwrap();

        let result = validate_path_not_symlink(&link);
        assert!(result.is_err(), "Symlink should fail validation");
        assert!(
            result.unwrap_err().to_string().contains("symbolic link"),
            "Error should mention symbolic link"
        );
    }

    #[test]
    fn test_validate_nonexistent_path() {
        let nonexistent = PathBuf::from("/tmp/does_not_exist_12345");
        let result = validate_path_not_symlink(&nonexistent);
        assert!(result.is_err(), "Nonexistent path should fail validation");
    }

    // ===== Edge Case Tests =====

    #[test]
    fn test_multiple_consecutive_slashes() {
        // Path with multiple consecutive slashes: /Users//test///project
        let path = PathBuf::from("/Users//test///project");
        let encoded = encode_path(&path);
        let decoded = decode_path(&encoded);

        // Path should be preserved (not normalized)
        assert_eq!(decoded, path, "Should preserve consecutive slashes");
        assert!(validate_decoded_path(&decoded).is_ok(), "Consecutive slashes should be valid");
    }

    #[test]
    fn test_trailing_slash_in_path() {
        // Test paths with trailing slashes
        let path_with_slash = PathBuf::from("/Users/test/project/");
        let path_without_slash = PathBuf::from("/Users/test/project");

        let encoded_with = encode_path(&path_with_slash);
        let encoded_without = encode_path(&path_without_slash);

        let decoded_with = decode_path(&encoded_with);
        let decoded_without = decode_path(&encoded_without);

        // Paths should be preserved as-is
        assert_eq!(decoded_with, path_with_slash, "Should preserve trailing slash");
        assert_eq!(decoded_without, path_without_slash, "Should preserve no trailing slash");

        // Both should be valid
        assert!(validate_decoded_path(&decoded_with).is_ok());
        assert!(validate_decoded_path(&decoded_without).is_ok());
    }

    #[test]
    fn test_path_at_os_limit() {
        // Test path at typical OS limit (4096 bytes on Linux)
        let long_component = "a".repeat(100);
        let mut path_str = String::from("/");

        // Add components until we reach ~4000 bytes
        while path_str.len() < 4000 {
            path_str.push_str(&long_component);
            path_str.push('/');
        }

        let long_path = PathBuf::from(&path_str);
        let encoded = encode_path(&long_path);
        let decoded = decode_path(&encoded);

        // Should handle very long paths
        assert_eq!(decoded, long_path, "Should handle paths at OS limits");
        assert!(validate_decoded_path(&decoded).is_ok(), "Very long paths should be valid");
    }

    #[test]
    #[cfg(unix)]
    fn test_hardlink_validation() {
        use std::io::Write;

        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();

        // Create original file
        let original = temp_dir.path().join("original.txt");
        let mut file = std::fs::File::create(&original).unwrap();
        file.write_all(b"test").unwrap();
        drop(file);

        // Create hardlink
        let hardlink = temp_dir.path().join("hardlink.txt");
        std::fs::hard_link(&original, &hardlink).unwrap();

        // Both should pass symlink validation (hardlinks are not symlinks)
        let result_original = validate_path_not_symlink(&original);
        let result_hardlink = validate_path_not_symlink(&hardlink);

        assert!(result_original.is_ok(), "Original file should pass");
        assert!(result_hardlink.is_ok(), "Hardlink should pass symlink check");

        // Note: Current implementation doesn't detect hardlinks
        // This test documents the current behavior (vulnerability)
        // To detect hardlinks, would need to check inode numbers and nlink count
    }

    #[test]
    fn test_path_only_dots() {
        // Path with only dots: /././.
        let path = PathBuf::from("/././.");
        let result = validate_decoded_path(&path);
        // Single dots (.) are allowed (current directory)
        assert!(result.is_ok(), "Path with only single dots should be valid");
    }

    #[test]
    fn test_paths_with_newlines() {
        // Path with newline character (unusual but possible in Unix)
        let path_with_newline = "/Users/test/project\nmalicious";
        let encoded = utf8_percent_encode(path_with_newline.strip_prefix('/').unwrap(), ENCODE_SET)
            .to_string();
        let formatted = format!("-{}", encoded);
        let decoded = decode_path(&formatted);

        // Newline should be encoded and preserved
        assert!(decoded.to_string_lossy().contains('\n'), "Newline should be preserved");
        // Validation should pass (no .. or relative path check)
        assert!(
            validate_decoded_path(&decoded).is_ok(),
            "Path with newline should pass validation"
        );
    }

    #[test]
    fn test_normalize_slash_handling() {
        // Test that /Users/foo/ and /Users/foo encode differently
        let with_slash = PathBuf::from("/Users/foo/");
        let without_slash = PathBuf::from("/Users/foo");

        let enc1 = encode_path(&with_slash);
        let enc2 = encode_path(&without_slash);

        // They should encode to different strings
        assert_ne!(enc1, enc2, "Trailing slash should affect encoding");
    }

    #[test]
    fn test_path_with_many_slashes_at_end() {
        // Path with many trailing slashes
        let path = PathBuf::from("/Users/test////");
        let encoded = encode_path(&path);
        let decoded = decode_path(&encoded);

        assert_eq!(decoded, path, "Should preserve multiple trailing slashes");
    }
}
