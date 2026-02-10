use crate::auth::types::{Workspace, WorkspaceAuth};
use crate::error::{AuthError, Result};
use regex::Regex;
use std::io::{self, Read};

/// Parse a cURL command from stdin and extract Slack credentials
///
/// Looks for:
/// - Workspace URL (https://*.slack.com)
/// - xoxd cookie (d=xoxd-...)
/// - xoxc token (in request body or -d flag)
pub fn parse_curl_from_stdin() -> Result<Workspace> {
    let mut buffer = String::new();
    io::stdin()
        .read_to_string(&mut buffer)
        .map_err(|e| AuthError::InvalidAuth(format!("Failed to read stdin: {}", e)))?;

    parse_curl_command(&buffer)
}

/// Parse a cURL command string and extract Slack credentials
pub fn parse_curl_command(curl_text: &str) -> Result<Workspace> {
    // Extract workspace URL
    let url_re = Regex::new(r#"(?:curl\s+['"]?|'|")(https://[a-zA-Z0-9-]+\.slack\.com)"#).unwrap();
    let workspace_url = url_re
        .captures(curl_text)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
        .ok_or_else(|| AuthError::InvalidAuth("No Slack workspace URL found in cURL command".to_string()))?;

    // Extract xoxd cookie (d=xoxd-...)
    // Can appear in --cookie, -H 'Cookie:', or similar
    let cookie_re = Regex::new(r#"d=(xoxd-[a-zA-Z0-9-]+)"#).unwrap();
    let xoxd_cookie = cookie_re
        .captures(curl_text)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
        .ok_or_else(|| AuthError::InvalidAuth("No xoxd cookie found in cURL command".to_string()))?;

    // Extract xoxc token (token=xoxc-... in request body)
    let token_re = Regex::new(r#"token=(xoxc-[a-zA-Z0-9-]+)"#).unwrap();
    let xoxc_token = token_re
        .captures(curl_text)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
        .ok_or_else(|| AuthError::InvalidAuth("No xoxc token found in cURL command".to_string()))?;

    Ok(Workspace {
        workspace_url,
        workspace_name: None,
        team_id: None,
        team_domain: None,
        auth: WorkspaceAuth::Browser {
            xoxc_token,
            xoxd_cookie,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_curl_command() {
        let curl = r#"curl 'https://myteam.slack.com/api/conversations.list' \
  -H 'cookie: d=xoxd-abc123def456; other=value' \
  --data-urlencode 'token=xoxc-xyz789' \
  -H 'accept: application/json'"#;

        let result = parse_curl_command(curl).unwrap();

        assert_eq!(result.workspace_url, "https://myteam.slack.com");
        match result.auth {
            WorkspaceAuth::Browser {
                xoxc_token,
                xoxd_cookie,
            } => {
                assert_eq!(xoxc_token, "xoxc-xyz789");
                assert_eq!(xoxd_cookie, "xoxd-abc123def456");
            }
            _ => panic!("Expected Browser auth"),
        }
    }

    #[test]
    fn test_parse_curl_with_data_flag() {
        let curl = r#"curl "https://example.slack.com/api/chat.postMessage" \
  -d "token=xoxc-test123" \
  -H "Cookie: d=xoxd-cookie456""#;

        let result = parse_curl_command(curl).unwrap();

        assert_eq!(result.workspace_url, "https://example.slack.com");
        match result.auth {
            WorkspaceAuth::Browser {
                xoxc_token,
                xoxd_cookie,
            } => {
                assert_eq!(xoxc_token, "xoxc-test123");
                assert_eq!(xoxd_cookie, "xoxd-cookie456");
            }
            _ => panic!("Expected Browser auth"),
        }
    }

    #[test]
    fn test_parse_curl_missing_url() {
        let curl = r#"curl 'https://example.com/api' \
  -H 'cookie: d=xoxd-abc123' \
  --data 'token=xoxc-xyz789'"#;

        let result = parse_curl_command(curl);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_curl_missing_cookie() {
        let curl = r#"curl 'https://myteam.slack.com/api/test' \
  --data 'token=xoxc-xyz789'"#;

        let result = parse_curl_command(curl);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_curl_missing_token() {
        let curl = r#"curl 'https://myteam.slack.com/api/test' \
  -H 'cookie: d=xoxd-abc123'"#;

        let result = parse_curl_command(curl);
        assert!(result.is_err());
    }
}
