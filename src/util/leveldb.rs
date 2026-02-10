use crate::error::Result;
use rusty_leveldb::{LdbIterator, Options, DB};
use std::path::Path;

/// LevelDB key-value entry
#[derive(Debug, Clone)]
pub struct LevelDBEntry {
    #[allow(dead_code)]
    pub key: Vec<u8>,
    pub value: Vec<u8>,
}

/// Scan a LevelDB database for keys matching a substring
///
/// This is specifically designed for reading Chromium LevelDB storage
/// (e.g., Slack Desktop's Local Storage).
///
/// Returns all key-value pairs where the key contains the given substring.
#[allow(dead_code)]
pub fn scan_leveldb_for_keys<P: AsRef<Path>>(
    db_path: P,
    key_substring: &[u8],
) -> Result<Vec<LevelDBEntry>> {
    let mut entries = Vec::new();

    // Open database in read-only mode
    let mut options = Options::default();
    // Set to read-only to avoid creating new files
    options.create_if_missing = false;

    let mut db = DB::open(db_path, options)
        .map_err(|e| crate::error::SlackersError::Other(format!("Failed to open LevelDB: {}", e)))?;

    // Iterate through all entries
    let mut iter = db.new_iter()
        .map_err(|e| crate::error::SlackersError::Other(format!("Failed to create iterator: {}", e)))?;

    // Scan all keys
    loop {
        match iter.next() {
            Some((key, value)) => {
                // Check if key contains the substring
                if contains_substring(&key, key_substring) {
                    entries.push(LevelDBEntry { key, value });
                }
            }
            None => break,
        }
    }

    Ok(entries)
}

/// Find all entries in a LevelDB database matching any of the given key substrings
///
/// This is useful for finding multiple related keys (e.g., "localConfig_v2" and "localConfig_v3").
pub fn scan_leveldb_for_keys_multi<P: AsRef<Path>>(
    db_path: P,
    key_substrings: &[&[u8]],
) -> Result<Vec<LevelDBEntry>> {
    let mut entries = Vec::new();

    // Open database in read-only mode
    let mut options = Options::default();
    options.create_if_missing = false;

    let mut db = DB::open(db_path, options)
        .map_err(|e| crate::error::SlackersError::Other(format!("Failed to open LevelDB: {}", e)))?;

    // Iterate through all entries
    let mut iter = db.new_iter()
        .map_err(|e| crate::error::SlackersError::Other(format!("Failed to create iterator: {}", e)))?;

    // Scan all keys
    loop {
        match iter.next() {
            Some((key, value)) => {
                // Check if key contains any of the substrings
                for substring in key_substrings {
                    if contains_substring(&key, substring) {
                        entries.push(LevelDBEntry { key, value });
                        break;
                    }
                }
            }
            None => break,
        }
    }

    Ok(entries)
}

/// Check if a slice contains a substring
fn contains_substring(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() {
        return true;
    }
    if haystack.len() < needle.len() {
        return false;
    }

    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contains_substring() {
        assert!(contains_substring(b"hello world", b"world"));
        assert!(contains_substring(b"localConfig_v2", b"localConfig"));
        assert!(!contains_substring(b"hello", b"world"));
        assert!(contains_substring(b"test", b""));
        assert!(!contains_substring(b"hi", b"hello"));
    }

    #[test]
    fn test_contains_substring_exact() {
        assert!(contains_substring(b"test", b"test"));
        assert!(contains_substring(b"test", b"es"));
        assert!(contains_substring(b"test", b"t"));
    }
}
