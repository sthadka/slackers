use crate::error::{Result, SlackersError};
use crate::util::scan_leveldb_for_keys_multi;
use aes::Aes128;
use cbc::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};
use pbkdf2::pbkdf2_hmac;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha1::Sha1;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

type Aes128CbcDec = cbc::Decryptor<Aes128>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopTeam {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub token: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DesktopExtracted {
    pub cookie_d: String,
    pub teams: Vec<DesktopTeam>,
    pub source: DesktopSource,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DesktopSource {
    pub leveldb_path: String,
    pub cookies_path: String,
}

/// Find Slack Desktop data directory
///
/// Checks both Electron (direct download) and Mac App Store (sandboxed) locations.
fn find_slack_data_dir() -> Result<(PathBuf, PathBuf)> {
    let home = dirs::home_dir().ok_or_else(|| SlackersError::Other("Cannot find home directory".to_string()))?;

    // Electron path: ~/Library/Application Support/Slack
    let electron_dir = home
        .join("Library")
        .join("Application Support")
        .join("Slack");

    // Mac App Store path (sandboxed)
    let appstore_dir = home
        .join("Library")
        .join("Containers")
        .join("com.tinyspeck.slackmacgap")
        .join("Data")
        .join("Library")
        .join("Application Support")
        .join("Slack");

    for base_dir in [electron_dir, appstore_dir] {
        let leveldb_dir = base_dir.join("Local Storage").join("leveldb");
        if leveldb_dir.exists() {
            let cookies_path = base_dir.join("Cookies");
            return Ok((leveldb_dir, cookies_path));
        }
    }

    Err(SlackersError::Other(
        "Slack Desktop data not found. Is Slack Desktop installed?".to_string(),
    ))
}

/// Snapshot LevelDB directory to temporary location
///
/// Uses copy-on-write on macOS (cp -cR) for speed, falls back to regular copy.
fn snapshot_leveldb<P: AsRef<Path>>(src_dir: P) -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| SlackersError::Other("Cannot find home directory".to_string()))?;

    let cache_base = home
        .join(".config")
        .join("slackers")
        .join("cache")
        .join("leveldb-snapshots");

    fs::create_dir_all(&cache_base)?;

    let dest = cache_base.join(format!("{}", chrono::Utc::now().timestamp_millis()));

    // Try copy-on-write on macOS for speed
    let cp_result = Command::new("cp")
        .args(["-cR", src_dir.as_ref().to_str().unwrap(), dest.to_str().unwrap()])
        .output();

    if cp_result.is_err() || !cp_result.as_ref().unwrap().status.success() {
        // Fallback to regular copy
        copy_dir_recursive(src_dir.as_ref(), &dest)?;
    }

    // Remove LOCK file if exists
    let lock_file = dest.join("LOCK");
    if lock_file.exists() {
        let _ = fs::remove_file(lock_file);
    }

    Ok(dest)
}

/// Recursively copy directory
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if ty.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}

/// Parse localConfig value from LevelDB
///
/// Handles encoding detection (UTF-8 vs UTF-16LE) and JSON extraction.
fn parse_local_config(raw: &[u8]) -> Result<Value> {
    if raw.is_empty() {
        return Err(SlackersError::Other("localConfig is empty".to_string()));
    }

    // Skip leading byte prefix if present (0x00, 0x01, 0x02)
    let data = if raw[0] <= 0x02 { &raw[1..] } else { raw };

    // Detect encoding by counting null bytes
    let nul_count = data.iter().filter(|&&b| b == 0).count();
    let encodings = if nul_count > data.len() / 4 {
        vec!["utf-16le", "utf-8"]
    } else {
        vec!["utf-8", "utf-16le"]
    };

    for encoding in encodings {
        // Try to decode
        let text = match encoding {
            "utf-8" => String::from_utf8_lossy(data).to_string(),
            "utf-16le" => {
                // Decode UTF-16LE
                let u16_vec: Vec<u16> = data
                    .chunks_exact(2)
                    .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
                    .collect();
                String::from_utf16_lossy(&u16_vec)
            }
            _ => continue,
        };

        // Try parsing as JSON
        if let Ok(value) = serde_json::from_str::<Value>(&text) {
            return Ok(value);
        }

        // Try extracting JSON substring
        if let Some(start) = text.find('{') {
            if let Some(end) = text.rfind('}') {
                if end > start {
                    if let Ok(value) = serde_json::from_str::<Value>(&text[start..=end]) {
                        return Ok(value);
                    }
                }
            }
        }
    }

    Err(SlackersError::Other("localConfig not parseable as JSON".to_string()))
}

/// Extract teams from Slack Desktop LevelDB
async fn extract_teams_from_leveldb<P: AsRef<Path>>(leveldb_dir: P) -> Result<Vec<DesktopTeam>> {
    if !leveldb_dir.as_ref().exists() {
        return Err(SlackersError::Other(format!(
            "Slack LevelDB not found: {}",
            leveldb_dir.as_ref().display()
        )));
    }

    // Snapshot the LevelDB directory
    let snap = snapshot_leveldb(&leveldb_dir)?;

    // Ensure cleanup on exit
    let result = (|| {
        // Search for localConfig_v2 and localConfig_v3
        let entries = scan_leveldb_for_keys_multi(
            &snap,
            &[b"localConfig_v2", b"localConfig_v3"],
        )?;

        // Find first non-empty config value
        let config_entry = entries
            .iter()
            .find(|e| !e.value.is_empty())
            .ok_or_else(|| SlackersError::Other("No localConfig_v2/v3 found in LevelDB".to_string()))?;

        // Parse the config
        let config = parse_local_config(&config_entry.value)?;

        // Extract teams
        let teams = config
            .get("teams")
            .and_then(|t| t.as_object())
            .ok_or_else(|| SlackersError::Other("No teams in localConfig".to_string()))?;

        let mut result_teams = Vec::new();
        for team_value in teams.values() {
            if let Some(team) = parse_team(team_value) {
                // Only include teams with xoxc- tokens
                if team.token.starts_with("xoxc-") {
                    result_teams.push(team);
                }
            }
        }

        if result_teams.is_empty() {
            return Err(SlackersError::Other("No xoxc tokens found in Slack localConfig".to_string()));
        }

        Ok(result_teams)
    })();

    // Cleanup snapshot
    let _ = fs::remove_dir_all(&snap);

    result
}

/// Parse a team object from JSON
fn parse_team(value: &Value) -> Option<DesktopTeam> {
    let obj = value.as_object()?;
    let url = obj.get("url")?.as_str()?.to_string();
    let token = obj.get("token")?.as_str()?.to_string();
    let name = obj.get("name").and_then(|n| n.as_str()).map(|s| s.to_string());

    Some(DesktopTeam { url, name, token })
}

/// Get Safe Storage password from macOS Keychain
fn get_safe_storage_password() -> Result<String> {
    let services = ["Slack Safe Storage", "Chrome Safe Storage", "Chromium Safe Storage"];

    for service in services {
        let output = Command::new("security")
            .args(["find-generic-password", "-w", "-s", service])
            .output();

        if let Ok(output) = output {
            if output.status.success() {
                let password = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !password.is_empty() {
                    return Ok(password);
                }
            }
        }
    }

    Err(SlackersError::Other(
        "Could not read Safe Storage password from Keychain (tried Slack/Chrome/Chromium Safe Storage)".to_string(),
    ))
}

/// Decrypt Chromium cookie value using PBKDF2 + AES-128-CBC
fn decrypt_chromium_cookie(encrypted: &[u8], password: &str) -> Result<String> {
    if encrypted.is_empty() {
        return Ok(String::new());
    }

    // Check for v10/v11 prefix
    let data = if encrypted.len() >= 3 {
        let prefix = &encrypted[0..3];
        if prefix == b"v10" || prefix == b"v11" {
            &encrypted[3..]
        } else {
            encrypted
        }
    } else {
        encrypted
    };

    // Chromium encryption parameters
    let salt = b"saltysalt";
    let iterations = 1003;
    let mut key = [0u8; 16];
    pbkdf2_hmac::<Sha1>(password.as_bytes(), salt, iterations, &mut key);

    let iv = [0x20u8; 16]; // 16 spaces

    // Decrypt using AES-128-CBC
    let mut buffer = data.to_vec();
    let decrypted = Aes128CbcDec::new(&key.into(), &iv.into())
        .decrypt_padded_mut::<Pkcs7>(&mut buffer)
        .map_err(|e| SlackersError::Other(format!("Decryption failed: {}", e)))?;

    // Find xoxd- token in decrypted data
    if let Some(pos) = decrypted.windows(5).position(|w| w == b"xoxd-") {
        // Find end of token (ASCII printable range)
        let mut end = pos;
        while end < decrypted.len() {
            let byte = decrypted[end];
            if byte < 0x21 || byte > 0x7e {
                break;
            }
            end += 1;
        }

        let token_bytes = &decrypted[pos..end];
        let token = String::from_utf8_lossy(token_bytes).to_string();

        // Try URL decoding
        match urlencoding::decode(&token) {
            Ok(decoded) => return Ok(decoded.to_string()),
            Err(_) => return Ok(token),
        }
    }

    // Fallback to full decrypted string
    Ok(String::from_utf8_lossy(decrypted).to_string())
}

/// Extract cookie 'd' from Slack Cookies database
fn extract_cookie_d<P: AsRef<Path>>(cookies_path: P) -> Result<String> {
    if !cookies_path.as_ref().exists() {
        return Err(SlackersError::Other(format!(
            "Slack Cookies DB not found: {}",
            cookies_path.as_ref().display()
        )));
    }

    let conn = Connection::open(cookies_path)?;

    let mut stmt = conn.prepare(
        "SELECT host_key, name, value, encrypted_value \
         FROM cookies \
         WHERE name = 'd' AND host_key LIKE '%slack.com' \
         ORDER BY length(encrypted_value) DESC"
    )?;

    let mut rows = stmt.query([])?;

    if let Some(row) = rows.next()? {
        // Try plaintext value first
        let value: Option<String> = row.get(2)?;
        if let Some(val) = value {
            if val.starts_with("xoxd-") {
                return Ok(val);
            }
        }

        // Decrypt encrypted_value
        let encrypted: Vec<u8> = row.get(3)?;
        if encrypted.is_empty() {
            return Err(SlackersError::Other("Slack 'd' cookie had no encrypted_value".to_string()));
        }

        let password = get_safe_storage_password()?;
        let decrypted = decrypt_chromium_cookie(&encrypted, &password)?;

        // Extract xoxd- token
        if let Some(start) = decrypted.find("xoxd-") {
            // Find end of token
            let token_part = &decrypted[start..];
            let end = token_part
                .find(|c: char| !c.is_ascii_alphanumeric() && !"-_=.%/+".contains(c))
                .unwrap_or(token_part.len());

            return Ok(token_part[..end].to_string());
        }

        return Err(SlackersError::Other("Could not locate xoxd-* in decrypted Slack cookie".to_string()));
    }

    Err(SlackersError::Other("No Slack 'd' cookie found".to_string()))
}

/// Extract authentication data from Slack Desktop
pub async fn extract_from_slack_desktop() -> Result<DesktopExtracted> {
    let (leveldb_dir, cookies_path) = find_slack_data_dir()?;
    let teams = extract_teams_from_leveldb(&leveldb_dir).await?;
    let cookie_d = extract_cookie_d(&cookies_path)?;

    Ok(DesktopExtracted {
        cookie_d,
        teams,
        source: DesktopSource {
            leveldb_path: leveldb_dir.to_string_lossy().to_string(),
            cookies_path: cookies_path.to_string_lossy().to_string(),
        },
    })
}
