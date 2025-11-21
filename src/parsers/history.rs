use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::models::HistoryEntry;
use crate::utils::validate_file_size;

/// Parse history.jsonl file and return list of history entries
/// Gracefully handles malformed lines by logging and skipping them
/// Returns an error if more than 50% of lines fail to parse or >100 consecutive errors
pub fn parse_history_file(path: &Path) -> Result<Vec<HistoryEntry>> {
    // Open file and validate size to avoid TOCTOU race condition
    let file = File::open(path)
        .with_context(|| format!("Failed to open history file: {}", path.display()))?;
    validate_file_size(&file, path)?;

    let reader = BufReader::new(file);
    let mut entries = Vec::new();
    let mut skipped_count = 0;
    let mut total_lines = 0;
    let mut consecutive_errors = 0;
    const MAX_CONSECUTIVE_ERRORS: usize = 100;

    for (line_num, line) in reader.lines().enumerate() {
        let line = line.context("Failed to read line from history file")?;

        // Skip empty lines
        if line.trim().is_empty() {
            continue;
        }

        total_lines += 1;

        match serde_json::from_str::<HistoryEntry>(&line) {
            Ok(entry) => {
                entries.push(entry);
                consecutive_errors = 0; // Reset on success
            }
            Err(e) => {
                eprintln!("Warning: Failed to parse line {} in history file: {}", line_num + 1, e);
                skipped_count += 1;
                consecutive_errors += 1;

                // Bail if too many consecutive errors
                if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                    bail!(
                        "Too many consecutive parse errors ({}) in history file - file may be corrupted",
                        consecutive_errors
                    );
                }
            }
        }
    }

    // Check if failure rate is too high
    if total_lines > 0 {
        let failure_rate = (skipped_count as f64) / (total_lines as f64);
        if failure_rate > 0.5 {
            bail!(
                "Too many parse failures in history file: {} of {} lines failed ({:.1}%)",
                skipped_count,
                total_lines,
                failure_rate * 100.0
            );
        }
    }

    if skipped_count > 0 {
        eprintln!("Parsed history file: {} entries ({} skipped)", entries.len(), skipped_count);
    }

    Ok(entries)
}
