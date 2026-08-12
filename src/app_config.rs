use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

/// User-facing config loaded from ~/.config/slackers/config.toml.
///
/// Example config.toml:
///
/// ```toml
/// [history]
/// auto_resume = true
/// exclude_subtypes = ["channel_join", "channel_leave", "channel_topic"]
/// exclude_users = ["USLACKBOT"]
/// ```
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct AppConfig {
    pub history: HistoryConfig,
    pub store: StoreConfig,
}

#[derive(Deserialize)]
#[serde(default)]
pub struct StoreConfig {
    pub enabled: bool,
    pub sync_scope: SyncScope,
    pub store_raw_json: bool,
    pub max_db_size_mb: Option<u32>,
    pub auto_gc: bool,
    pub defaults: StoreDefaults,
}

impl Default for StoreConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            sync_scope: SyncScope::default(),
            store_raw_json: false,
            max_db_size_mb: None,
            auto_gc: true,
            defaults: StoreDefaults::default(),
        }
    }
}

#[derive(Deserialize)]
#[serde(default)]
pub struct StoreDefaults {
    pub retention_days: Option<u32>,
    pub sync_threads: bool,
    pub sync_members: bool,
    pub sync_files: bool,
    pub max_file_size_mb: Option<u32>,
}

impl Default for StoreDefaults {
    fn default() -> Self {
        Self {
            retention_days: Some(90),
            sync_threads: true,
            sync_members: false,
            sync_files: false,
            max_file_size_mb: Some(10),
        }
    }
}

#[derive(Deserialize, Default, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SyncScope {
    #[default]
    Public,
    PublicPrivate,
    All,
    Selected,
}

#[derive(Deserialize)]
#[serde(default)]
pub struct HistoryConfig {
    /// Resume an interrupted `message history` run automatically.
    /// When true (the default), slackers saves the oldest ts after each
    /// fetched page and picks it up on the next run as --before.
    pub auto_resume: bool,

    /// Slack message subtypes to drop from output.
    /// Common values: "channel_join", "channel_leave", "channel_topic",
    /// "channel_purpose", "bot_message", "channel_archive".
    pub exclude_subtypes: Vec<String>,

    /// Slack user IDs (U...) whose messages to drop from output.
    pub exclude_users: Vec<String>,
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            auto_resume: true,
            exclude_subtypes: Vec::new(),
            exclude_users: Vec::new(),
        }
    }
}

/// Load config from ~/.config/slackers/config.toml.
/// Returns defaults silently on missing file or parse errors.
pub fn load_app_config() -> AppConfig {
    let path = match config_toml_path() {
        Ok(p) => p,
        Err(_) => return AppConfig::default(),
    };
    if !path.exists() {
        return AppConfig::default();
    }
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return AppConfig::default(),
    };
    toml::from_str(&content).unwrap_or_default()
}

/// ~/.config/slackers/config.toml
pub fn config_toml_path() -> Result<PathBuf, ()> {
    let dir = dirs::config_dir().ok_or(())?;
    Ok(dir.join("slackers").join("config.toml"))
}

/// ~/.config/slackers/history-cursors.json
/// Stores { channel_id -> oldest_ts_fetched } for auto-resume.
pub fn history_cursors_path() -> Result<PathBuf, ()> {
    let dir = dirs::config_dir().ok_or(())?;
    Ok(dir.join("slackers").join("history-cursors.json"))
}

/// ~/.config/slackers/channel-cache.json
/// Stores { channel_name -> channel_id } to avoid repeated conversations.list pagination.
pub fn channel_cache_path() -> Result<PathBuf, ()> {
    let dir = dirs::config_dir().ok_or(())?;
    Ok(dir.join("slackers").join("channel-cache.json"))
}

pub fn load_channel_cache() -> HashMap<String, String> {
    let Ok(path) = channel_cache_path() else {
        return HashMap::new();
    };
    let Ok(content) = std::fs::read_to_string(&path) else {
        return HashMap::new();
    };
    serde_json::from_str(&content).unwrap_or_default()
}

pub fn save_channel_cache(cache: &HashMap<String, String>) {
    let Ok(path) = channel_cache_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(content) = serde_json::to_string_pretty(cache) {
        let _ = std::fs::write(path, content);
    }
}

/// Load the saved resume-cursor map from disk (silently returns empty on error).
pub fn load_history_cursors() -> HashMap<String, String> {
    let Ok(path) = history_cursors_path() else {
        return HashMap::new();
    };
    let Ok(content) = std::fs::read_to_string(&path) else {
        return HashMap::new();
    };
    serde_json::from_str(&content).unwrap_or_default()
}

/// Persist the resume-cursor map to disk (silently ignores errors).
pub fn save_history_cursors(cursors: &HashMap<String, String>) {
    let Ok(path) = history_cursors_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(content) = serde_json::to_string_pretty(cursors) {
        let _ = std::fs::write(path, content);
    }
}
