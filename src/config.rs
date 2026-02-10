use crate::error::{ConfigError, Result};
use std::path::PathBuf;

/// Get the credentials file path: ~/.config/slackers/credentials.json
pub fn credentials_path() -> Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .ok_or(ConfigError::DirectoryNotFound)?;

    Ok(config_dir.join("slackers").join("credentials.json"))
}

/// Get the downloads directory: ~/.slackers/tmp/downloads/
pub fn downloads_dir() -> Result<PathBuf> {
    let home_dir = dirs::home_dir()
        .ok_or(ConfigError::DirectoryNotFound)?;

    Ok(home_dir.join(".slackers").join("tmp").join("downloads"))
}

/// Get the LevelDB cache directory: ~/.config/slackers/cache/leveldb-snapshots/
#[allow(dead_code)]
pub fn leveldb_cache_dir() -> Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .ok_or(ConfigError::DirectoryNotFound)?;

    Ok(config_dir.join("slackers").join("cache").join("leveldb-snapshots"))
}

/// Ensure the parent directory exists for a given path
pub fn ensure_parent_dir(path: &PathBuf) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| ConfigError::WriteError(e.to_string()))?;
    }
    Ok(())
}

/// Ensure a directory exists
pub fn ensure_dir(path: &PathBuf) -> Result<()> {
    std::fs::create_dir_all(path)
        .map_err(|e| ConfigError::WriteError(e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_credentials_path() {
        let path = credentials_path().unwrap();
        assert!(path.to_string_lossy().contains("slackers"));
        assert!(path.to_string_lossy().ends_with("credentials.json"));
    }

    #[test]
    fn test_downloads_dir() {
        let path = downloads_dir().unwrap();
        assert!(path.to_string_lossy().contains(".slackers"));
        assert!(path.to_string_lossy().contains("tmp"));
        assert!(path.to_string_lossy().contains("downloads"));
    }

    #[test]
    fn test_leveldb_cache_dir() {
        let path = leveldb_cache_dir().unwrap();
        assert!(path.to_string_lossy().contains("slackers"));
        assert!(path.to_string_lossy().contains("cache"));
        assert!(path.to_string_lossy().contains("leveldb-snapshots"));
    }
}
