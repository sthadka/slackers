use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credentials {
    #[serde(default = "default_version")]
    pub version: u32,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_workspace_url: Option<String>,

    #[serde(default)]
    pub workspaces: Vec<Workspace>,
}

fn default_version() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub workspace_url: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub team_id: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub team_domain: Option<String>,

    pub auth: WorkspaceAuth,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "auth_type")]
pub enum WorkspaceAuth {
    #[serde(rename = "standard")]
    Standard { token: String },

    #[serde(rename = "browser")]
    Browser {
        xoxc_token: String,
        xoxd_cookie: String,
    },
}

impl Credentials {
    pub fn new() -> Self {
        Self {
            version: 1,
            updated_at: None,
            default_workspace_url: None,
            workspaces: Vec::new(),
        }
    }
}

impl Default for Credentials {
    fn default() -> Self {
        Self::new()
    }
}

/// Normalize a workspace URL to "{protocol}//{host}" format
///
/// Examples:
/// - "https://myteam.slack.com" -> "https://myteam.slack.com"
/// - "https://myteam.slack.com/archives/C123" -> "https://myteam.slack.com"
/// - "myteam.slack.com" -> "https://myteam.slack.com"
pub fn normalize_workspace_url(url: &str) -> Result<String, String> {
    let url_str = if url.starts_with("http://") || url.starts_with("https://") {
        url.to_string()
    } else {
        format!("https://{}", url)
    };

    let parsed = Url::parse(&url_str)
        .map_err(|e| format!("Invalid URL '{}': {}", url, e))?;

    let host = parsed.host_str()
        .ok_or_else(|| format!("No host in URL: {}", url))?;

    let scheme = parsed.scheme();

    // Include port if present
    let url_with_port = if let Some(port) = parsed.port() {
        format!("{}://{}:{}", scheme, host, port)
    } else {
        format!("{}://{}", scheme, host)
    };

    Ok(url_with_port)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_workspace_url() {
        assert_eq!(
            normalize_workspace_url("https://myteam.slack.com").unwrap(),
            "https://myteam.slack.com"
        );

        assert_eq!(
            normalize_workspace_url("https://myteam.slack.com/archives/C123").unwrap(),
            "https://myteam.slack.com"
        );

        assert_eq!(
            normalize_workspace_url("myteam.slack.com").unwrap(),
            "https://myteam.slack.com"
        );

        assert_eq!(
            normalize_workspace_url("http://localhost:3000").unwrap(),
            "http://localhost:3000"
        );
    }

    #[test]
    fn test_credentials_serialization() {
        let creds = Credentials {
            version: 1,
            updated_at: Some("2024-01-01".to_string()),
            default_workspace_url: Some("https://test.slack.com".to_string()),
            workspaces: vec![
                Workspace {
                    workspace_url: "https://test.slack.com".to_string(),
                    workspace_name: Some("Test".to_string()),
                    team_id: Some("T123".to_string()),
                    team_domain: Some("test".to_string()),
                    auth: WorkspaceAuth::Standard {
                        token: "xoxb-test".to_string(),
                    },
                },
            ],
        };

        let json = serde_json::to_string_pretty(&creds).unwrap();
        let deserialized: Credentials = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.version, 1);
        assert_eq!(deserialized.workspaces.len(), 1);
    }

    #[test]
    fn test_workspace_auth_serialization() {
        let standard = WorkspaceAuth::Standard {
            token: "xoxb-123".to_string(),
        };
        let json = serde_json::to_string(&standard).unwrap();
        assert!(json.contains("\"auth_type\":\"standard\""));
        assert!(json.contains("\"token\":\"xoxb-123\""));

        let browser = WorkspaceAuth::Browser {
            xoxc_token: "xoxc-123".to_string(),
            xoxd_cookie: "xoxd-456".to_string(),
        };
        let json = serde_json::to_string(&browser).unwrap();
        assert!(json.contains("\"auth_type\":\"browser\""));
        assert!(json.contains("\"xoxc_token\":\"xoxc-123\""));
        assert!(json.contains("\"xoxd_cookie\":\"xoxd-456\""));
    }
}
