use crate::error::{Result, SlackersError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChromeTeam {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub token: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChromeExtracted {
    pub cookie_d: String,
    pub teams: Vec<ChromeTeam>,
}

/// Escape string for use in osascript -e '...'
fn escape_osascript(script: &str) -> String {
    script.replace('\'', r#"'"'"'"#)
}

/// Execute osascript with the given AppleScript
fn osascript(script: &str) -> Result<String> {
    let escaped = escape_osascript(script);
    let output = Command::new("osascript")
        .args(["-e", &escaped])
        .output()?;

    if !output.status.success() {
        return Err(SlackersError::Other(format!(
            "osascript failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// AppleScript to extract cookie 'd' from Chrome
fn cookie_script() -> &'static str {
    r#"
    tell application "Google Chrome"
      repeat with w in windows
        repeat with t in tabs of w
          if URL of t contains "slack.com" then
            return execute t javascript "document.cookie.split('; ').find(c => c.startsWith('d='))?.split('=')[1] || ''"
          end if
        end repeat
      end repeat
      return ""
    end tell
    "#
}

/// AppleScript to extract teams from Chrome localStorage
fn teams_script() -> String {
    let team_json_paths = vec![
        "JSON.stringify(JSON.parse(localStorage.localConfig_v2).teams)",
        "JSON.stringify(JSON.parse(localStorage.localConfig_v3).teams)",
        "JSON.stringify(JSON.parse(localStorage.getItem('reduxPersist:localConfig'))?.teams || {})",
        "JSON.stringify(window.boot_data?.teams || {})",
    ];

    let try_paths: Vec<String> = team_json_paths
        .iter()
        .map(|expr| format!("try {{ var v = {}; if (v && v !== '{{}}' && v !== 'null') return v; }} catch(e) {{}}", expr))
        .collect();

    format!(
        r#"
    tell application "Google Chrome"
      repeat with w in windows
        repeat with t in tabs of w
          if URL of t contains "slack.com" then
            return execute t javascript "(function(){{ {} return '{{}}'; }})()"
          end if
        end repeat
      end repeat
      return "{{}}"
    end tell
    "#,
        try_paths.join(" ")
    )
}

/// Parse a team object from JSON
fn parse_team(value: &Value) -> Option<ChromeTeam> {
    let obj = value.as_object()?;
    let url = obj.get("url")?.as_str()?.to_string();
    let token = obj.get("token")?.as_str()?.to_string();

    // Only include teams with xoxc- tokens
    if !token.starts_with("xoxc-") {
        return None;
    }

    let name = obj.get("name").and_then(|n| n.as_str()).map(|s| s.to_string());

    Some(ChromeTeam { url, name, token })
}

/// Extract authentication data from Chrome
///
/// Uses osascript to execute JavaScript in Chrome tabs with slack.com open.
/// Returns None if Chrome is not available or no Slack tabs are found.
pub fn extract_from_chrome() -> Result<Option<ChromeExtracted>> {
    // Check if running on macOS
    #[cfg(not(target_os = "macos"))]
    {
        return Ok(None);
    }

    #[cfg(target_os = "macos")]
    {
        // Try to extract cookie
        let cookie = osascript(cookie_script()).ok().unwrap_or_default();

        if cookie.is_empty() || !cookie.starts_with("xoxd-") {
            return Ok(None);
        }

        // Try to extract teams
        let teams_raw = osascript(&teams_script()).ok().unwrap_or_else(|| "{}".to_string());

        let teams_obj: Value = serde_json::from_str(&teams_raw).unwrap_or_else(|_| serde_json::json!({}));

        let teams: Vec<ChromeTeam> = if let Some(teams_map) = teams_obj.as_object() {
            teams_map
                .values()
                .filter_map(parse_team)
                .collect()
        } else {
            vec![]
        };

        if teams.is_empty() {
            return Ok(None);
        }

        Ok(Some(ChromeExtracted {
            cookie_d: cookie,
            teams,
        }))
    }
}
