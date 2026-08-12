use crate::error::{ConfigError, Result};
use crate::store::Store;
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

/// Hash a workspace URL to a 16-hex-char string for use in file names.
/// Uses the hostname portion of the URL to ensure consistency across
/// trailing slashes and path variations.
pub fn hash_workspace_url(workspace_url: &str) -> String {
    let trimmed = workspace_url.trim();
    if trimmed.is_empty() {
        return "unknown".to_string();
    }

    let source = if let Ok(url) = url::Url::parse(trimmed) {
        url.host_str()
            .unwrap_or(trimmed)
            .to_lowercase()
    } else {
        trimmed.to_lowercase()
    };

    if source.is_empty() || source == "unknown" {
        return "unknown".to_string();
    }

    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Get the store database path: ~/.local/share/slackers/store-{workspace_hash}.db
pub fn store_db_path(workspace_url: &str) -> Result<PathBuf> {
    let data_dir = dirs::data_dir()
        .ok_or(ConfigError::DirectoryNotFound)?;
    let hash = hash_workspace_url(workspace_url);
    Ok(data_dir.join("slackers").join(format!("store-{}.db", hash)))
}

/// Open the SQLite store if the `[store] enabled = true` setting is active.
/// Returns `Ok(None)` when the store is disabled (the default).
pub fn open_store_if_enabled(workspace_url: &str) -> Result<Option<Store>> {
    let config = crate::app_config::load_app_config();
    if !config.store.enabled {
        return Ok(None);
    }
    let path = store_db_path(workspace_url)?;
    Ok(Some(Store::open(&path)?))
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

    #[test]
    fn test_hash_workspace_url() {
        let hash1 = hash_workspace_url("https://myteam.slack.com");
        let hash2 = hash_workspace_url("https://myteam.slack.com/");
        assert_eq!(hash1, hash2);
        assert_ne!(hash1, "unknown");
        assert_eq!(hash1.len(), 16);

        assert_eq!(hash_workspace_url(""), "unknown");
        assert_eq!(hash_workspace_url("  "), "unknown");
    }

    #[test]
    fn test_store_db_path() {
        let path = store_db_path("https://myteam.slack.com").unwrap();
        assert!(path.to_string_lossy().contains("slackers"));
        assert!(path.to_string_lossy().contains("store-"));
        assert!(path.to_string_lossy().ends_with(".db"));
    }
}
