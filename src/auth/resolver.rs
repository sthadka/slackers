use crate::auth::store::{load_credentials, resolve_default_workspace, resolve_workspace_for_url};
use crate::auth::types::{normalize_workspace_url, WorkspaceAuth};
use crate::error::{AuthError, Result};
use std::env;

/// Resolved authentication with optional workspace context
#[derive(Debug, Clone)]
pub struct ResolvedAuth {
    pub auth: WorkspaceAuth,
    pub workspace_url: Option<String>,
}

/// Resolve authentication using priority chain:
/// 1. SLACK_TOKEN env var (+ SLACK_COOKIE_D/SLACK_COOKIE if xoxc)
/// 2. Stored credential by workspace URL
/// 3. Default workspace from credentials
/// 4. Auto-extract from Slack Desktop (macOS) - TODO: Phase 6
/// 5. Fallback Chrome extraction (macOS) - TODO: Phase 6
/// 6. Error with helpful message
pub fn resolve_auth(workspace_url: Option<&str>) -> Result<ResolvedAuth> {
    // 1. Check environment variables first
    if let Some(auth) = try_env_auth()? {
        return Ok(auth);
    }

    // 2. Try stored credential by workspace URL if provided
    if let Some(url) = workspace_url {
        if let Ok(workspace) = resolve_workspace_for_url(url) {
            let normalized = normalize_workspace_url(&workspace.workspace_url).ok();
            return Ok(ResolvedAuth {
                auth: workspace.auth,
                workspace_url: normalized,
            });
        }
    }

    // 3. Try default workspace
    if let Ok(workspace) = resolve_default_workspace() {
        let normalized = normalize_workspace_url(&workspace.workspace_url).ok();
        return Ok(ResolvedAuth {
            auth: workspace.auth,
            workspace_url: normalized,
        });
    }

    // 4. Auto-extract from Slack Desktop (macOS) - Phase 6
    // TODO: Implement in Phase 6
    // if cfg!(target_os = "macos") {
    //     if let Ok(extracted) = extract_from_desktop() {
    //         return Ok(extracted);
    //     }
    // }

    // 5. Fallback: Chrome extraction (macOS) - Phase 6
    // TODO: Implement in Phase 6
    // if cfg!(target_os = "macos") {
    //     if let Ok(extracted) = extract_from_chrome() {
    //         return Ok(extracted);
    //     }
    // }

    // 6. No credentials found
    Err(AuthError::NoCredentials.into())
}

/// Try to construct auth from environment variables
fn try_env_auth() -> Result<Option<ResolvedAuth>> {
    if let Ok(token) = env::var("SLACK_TOKEN") {
        let token = token.trim();

        // Check if it's a browser token (xoxc-)
        if token.starts_with("xoxc-") {
            // Need both xoxc and xoxd for browser auth
            let xoxd = env::var("SLACK_COOKIE_D")
                .or_else(|_| env::var("SLACK_COOKIE"))
                .map_err(|_| {
                    AuthError::InvalidAuth(
                        "SLACK_TOKEN is a browser token (xoxc-) but SLACK_COOKIE_D or SLACK_COOKIE not set"
                            .to_string(),
                    )
                })?;

            return Ok(Some(ResolvedAuth {
                auth: WorkspaceAuth::Browser {
                    xoxc_token: token.to_string(),
                    xoxd_cookie: xoxd.trim().to_string(),
                },
                workspace_url: None,
            }));
        }

        // Standard token (xoxb-, xoxp-, etc.)
        return Ok(Some(ResolvedAuth {
            auth: WorkspaceAuth::Standard {
                token: token.to_string(),
            },
            workspace_url: None,
        }));
    }

    Ok(None)
}

/// Get the effective workspace URL, used for resolving credentials
#[allow(dead_code)]
pub fn effective_workspace_url(provided: Option<&str>) -> Option<String> {
    if let Some(url) = provided {
        normalize_workspace_url(url).ok()
    } else {
        // Try to get default from config
        load_credentials()
            .ok()
            .and_then(|c| c.default_workspace_url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Mutex to ensure env var tests don't run in parallel
    static ENV_TEST_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn test_env_auth_standard_token() {
        let _lock = ENV_TEST_MUTEX.lock().unwrap();
        // Clean up any env vars from other tests
        env::remove_var("SLACK_TOKEN");
        env::remove_var("SLACK_COOKIE_D");
        env::remove_var("SLACK_COOKIE");

        env::set_var("SLACK_TOKEN", "xoxb-test-token");
        let result = try_env_auth().unwrap();
        assert!(result.is_some());
        let auth = result.unwrap();
        match auth.auth {
            WorkspaceAuth::Standard { token } => {
                assert_eq!(token, "xoxb-test-token");
            }
            _ => panic!("Expected standard auth"),
        }

        // Clean up
        env::remove_var("SLACK_TOKEN");
    }

    #[test]
    fn test_env_auth_browser_token() {
        let _lock = ENV_TEST_MUTEX.lock().unwrap();
        // Clean up any env vars from other tests
        env::remove_var("SLACK_TOKEN");
        env::remove_var("SLACK_COOKIE_D");
        env::remove_var("SLACK_COOKIE");

        env::set_var("SLACK_TOKEN", "xoxc-test-token");
        env::set_var("SLACK_COOKIE_D", "xoxd-test-cookie");
        let result = try_env_auth().unwrap();
        assert!(result.is_some());
        let auth = result.unwrap();
        match auth.auth {
            WorkspaceAuth::Browser {
                xoxc_token,
                xoxd_cookie,
            } => {
                assert_eq!(xoxc_token, "xoxc-test-token");
                assert_eq!(xoxd_cookie, "xoxd-test-cookie");
            }
            _ => panic!("Expected browser auth"),
        }
        env::remove_var("SLACK_TOKEN");
        env::remove_var("SLACK_COOKIE_D");
    }
}
