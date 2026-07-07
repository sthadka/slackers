use crate::slack::users::{get_user, CompactSlackUser};
use crate::slack::SlackClient;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::LazyLock;

const CACHE_VERSION: u32 = 1;
const USER_TTL_MS: u64 = 24 * 60 * 60 * 1000;

static USER_ID_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[UW][A-Z0-9]{8,}$").unwrap());
static USER_MENTION_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<@((?:U|W)[A-Z0-9]{8,})(?:\|[^>]+)?>").unwrap());

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UserCacheEntry {
    fetched_at: u64,
    user: CompactSlackUser,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UserCacheFile {
    version: u32,
    entries: HashMap<String, UserCacheEntry>,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn hash_workspace_url(workspace_url: &str) -> String {
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

fn cache_path(workspace_url: &str) -> Option<PathBuf> {
    let key = hash_workspace_url(workspace_url);
    if key == "unknown" {
        return None;
    }
    let data_dir = dirs::data_dir()?;
    let cache_dir = data_dir.join("slackers");
    Some(cache_dir.join(format!("users-cache-{}.json", key)))
}

fn load_cache(path: &std::path::Path) -> UserCacheFile {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return UserCacheFile { version: CACHE_VERSION, entries: HashMap::new() },
    };

    match serde_json::from_str::<UserCacheFile>(&content) {
        Ok(file) if file.version == CACHE_VERSION => file,
        _ => UserCacheFile { version: CACHE_VERSION, entries: HashMap::new() },
    }
}

fn write_cache(path: &std::path::Path, file: &UserCacheFile) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(file) {
        let _ = std::fs::write(path, json);
    }
}

fn prune_expired(file: &mut UserCacheFile, now: u64) {
    file.entries.retain(|_, entry| now - entry.fetched_at < USER_TTL_MS);
}

fn is_valid_user_id(s: &str) -> bool {
    USER_ID_PATTERN.is_match(s)
}

fn dedup_user_ids(ids: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    ids.iter()
        .filter(|id| is_valid_user_id(id) && seen.insert(id.to_string()))
        .cloned()
        .collect()
}

pub async fn resolve_users_by_id(
    client: &SlackClient,
    workspace_url: Option<&str>,
    user_ids: &[String],
    force_refresh: bool,
) -> HashMap<String, CompactSlackUser> {
    let unique_ids = dedup_user_ids(user_ids);
    if unique_ids.is_empty() {
        return HashMap::new();
    }

    let now = now_ms();
    let cp = workspace_url.and_then(|u| cache_path(u));
    let mut disk_cache = cp.as_ref().map(|p| load_cache(p)).unwrap_or(UserCacheFile {
        version: CACHE_VERSION,
        entries: HashMap::new(),
    });

    let mut out = HashMap::new();
    let mut missing = Vec::new();

    for uid in &unique_ids {
        if !force_refresh {
            if let Some(entry) = disk_cache.entries.get(uid) {
                if now - entry.fetched_at < USER_TTL_MS {
                    out.insert(uid.clone(), entry.user.clone());
                    continue;
                }
            }
        }
        missing.push(uid.clone());
    }

    let mut cache_changed = false;

    for uid in &missing {
        if let Ok(user) = get_user(client, uid).await {
            disk_cache.entries.insert(uid.clone(), UserCacheEntry {
                fetched_at: now,
                user: user.clone(),
            });
            out.insert(uid.clone(), user);
            cache_changed = true;
        }
    }

    if let Some(ref p) = cp {
        let orig_len = disk_cache.entries.len();
        prune_expired(&mut disk_cache, now);
        if disk_cache.entries.len() != orig_len {
            cache_changed = true;
        }
        if cache_changed {
            write_cache(p, &disk_cache);
        }
    }

    out
}

pub fn collect_referenced_user_ids(messages: &[serde_json::Value]) -> Vec<String> {
    let mut ids = HashSet::new();
    for msg in messages {
        collect_user_ids_from_value(msg, &mut ids);
    }
    ids.into_iter().collect()
}

fn collect_user_ids_from_value(value: &serde_json::Value, out: &mut HashSet<String>) {
    match value {
        serde_json::Value::String(s) => {
            for cap in USER_MENTION_PATTERN.captures_iter(s) {
                if let Some(m) = cap.get(1) {
                    let uid = m.as_str();
                    if is_valid_user_id(uid) {
                        out.insert(uid.to_string());
                    }
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                collect_user_ids_from_value(item, out);
            }
        }
        serde_json::Value::Object(obj) => {
            for (key, child) in obj {
                if key == "user" || key == "user_id" {
                    if let Some(s) = child.as_str() {
                        if is_valid_user_id(s) {
                            out.insert(s.to_string());
                        }
                    }
                    continue;
                }
                if key == "users" {
                    if let Some(arr) = child.as_array() {
                        for item in arr {
                            if let Some(s) = item.as_str() {
                                if is_valid_user_id(s) {
                                    out.insert(s.to_string());
                                }
                            }
                        }
                    }
                    continue;
                }
                collect_user_ids_from_value(child, out);
            }
        }
        _ => {}
    }
}

pub fn to_referenced_users(
    user_ids: &[String],
    users_by_id: &HashMap<String, CompactSlackUser>,
) -> Option<HashMap<String, CompactSlackUser>> {
    let mut out = HashMap::new();
    for uid in dedup_user_ids(user_ids) {
        if let Some(user) = users_by_id.get(&uid) {
            out.insert(uid, user.clone());
        }
    }
    if out.is_empty() { None } else { Some(out) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_is_valid_user_id() {
        assert!(is_valid_user_id("U0123456789"));
        assert!(is_valid_user_id("W0123456789"));
        assert!(!is_valid_user_id("C0123456789"));
        assert!(!is_valid_user_id("U123"));
        assert!(!is_valid_user_id(""));
    }

    #[test]
    fn test_dedup_user_ids() {
        let ids = vec![
            "U0123456789".to_string(),
            "U0123456789".to_string(),
            "UAABBCCDDEE".to_string(),
            "invalid".to_string(),
        ];
        let result = dedup_user_ids(&ids);
        assert_eq!(result.len(), 2);
        assert!(result.contains(&"U0123456789".to_string()));
        assert!(result.contains(&"UAABBCCDDEE".to_string()));
    }

    #[test]
    fn test_hash_workspace_url() {
        let hash1 = hash_workspace_url("https://myteam.slack.com");
        let hash2 = hash_workspace_url("https://myteam.slack.com/");
        assert_eq!(hash1, hash2);
        assert_ne!(hash1, "unknown");

        assert_eq!(hash_workspace_url(""), "unknown");
        assert_eq!(hash_workspace_url("  "), "unknown");
    }

    #[test]
    fn test_collect_referenced_user_ids_from_text() {
        let messages = vec![json!({
            "user": "U0123456789",
            "text": "Hey <@UAABBCCDDEE|john>, check this out"
        })];
        let ids = collect_referenced_user_ids(&messages);
        assert!(ids.contains(&"U0123456789".to_string()));
        assert!(ids.contains(&"UAABBCCDDEE".to_string()));
    }

    #[test]
    fn test_collect_referenced_user_ids_from_blocks() {
        let messages = vec![json!({
            "user": "U0123456789",
            "blocks": [
                {
                    "type": "rich_text",
                    "elements": [
                        {
                            "type": "user",
                            "user_id": "UAABBCCDDEE"
                        }
                    ]
                }
            ]
        })];
        let ids = collect_referenced_user_ids(&messages);
        assert!(ids.contains(&"U0123456789".to_string()));
        assert!(ids.contains(&"UAABBCCDDEE".to_string()));
    }

    #[test]
    fn test_collect_referenced_user_ids_empty() {
        let messages = vec![json!({
            "text": "No user references here"
        })];
        let ids = collect_referenced_user_ids(&messages);
        assert!(ids.is_empty());
    }

    #[test]
    fn test_to_referenced_users() {
        let mut users_by_id = HashMap::new();
        users_by_id.insert("U0123456789".to_string(), CompactSlackUser {
            id: "U0123456789".to_string(),
            name: Some("john".to_string()),
            real_name: Some("John Doe".to_string()),
            display_name: Some("johnd".to_string()),
            email: None,
            title: None,
            tz: None,
            is_bot: None,
            deleted: None,
        });

        let ids = vec!["U0123456789".to_string(), "UMISSING00000".to_string()];
        let result = to_referenced_users(&ids, &users_by_id);
        assert!(result.is_some());
        let map = result.unwrap();
        assert_eq!(map.len(), 1);
        assert!(map.contains_key("U0123456789"));
    }

    #[test]
    fn test_to_referenced_users_empty() {
        let users_by_id = HashMap::new();
        let ids = vec!["UMISSING00000".to_string()];
        let result = to_referenced_users(&ids, &users_by_id);
        assert!(result.is_none());
    }

    #[test]
    fn test_cache_file_roundtrip() {
        let mut entries = HashMap::new();
        entries.insert("U0123456789".to_string(), UserCacheEntry {
            fetched_at: 1000000,
            user: CompactSlackUser {
                id: "U0123456789".to_string(),
                name: Some("test".to_string()),
                real_name: None,
                display_name: None,
                email: None,
                title: None,
                tz: None,
                is_bot: None,
                deleted: None,
            },
        });
        let file = UserCacheFile {
            version: CACHE_VERSION,
            entries,
        };
        let json = serde_json::to_string(&file).unwrap();
        let parsed: UserCacheFile = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.version, CACHE_VERSION);
        assert!(parsed.entries.contains_key("U0123456789"));
    }

    #[test]
    fn test_prune_expired() {
        let now = 100_000_000;
        let mut file = UserCacheFile {
            version: CACHE_VERSION,
            entries: HashMap::new(),
        };
        file.entries.insert("U_FRESH0000000".to_string(), UserCacheEntry {
            fetched_at: now - 1000,
            user: CompactSlackUser {
                id: "U_FRESH0000000".to_string(),
                name: None,
                real_name: None,
                display_name: None,
                email: None,
                title: None,
                tz: None,
                is_bot: None,
                deleted: None,
            },
        });
        file.entries.insert("U_OLD000000000".to_string(), UserCacheEntry {
            fetched_at: now - USER_TTL_MS - 1,
            user: CompactSlackUser {
                id: "U_OLD000000000".to_string(),
                name: None,
                real_name: None,
                display_name: None,
                email: None,
                title: None,
                tz: None,
                is_bot: None,
                deleted: None,
            },
        });

        prune_expired(&mut file, now);
        assert_eq!(file.entries.len(), 1);
        assert!(file.entries.contains_key("U_FRESH0000000"));
    }

    #[test]
    fn test_mention_pattern_extraction() {
        let mut ids = HashSet::new();
        let val = json!("<@U0123456789> said hello to <@WAABBCCDDEE|bob>");
        collect_user_ids_from_value(&val, &mut ids);
        assert!(ids.contains("U0123456789"));
        assert!(ids.contains("WAABBCCDDEE"));
    }
}
