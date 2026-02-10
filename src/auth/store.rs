use crate::auth::keychain::{
    keychain_account, keychain_get, keychain_set, KEYCHAIN_PLACEHOLDER, KEYCHAIN_SERVICE,
};
use crate::auth::types::{normalize_workspace_url, Credentials, Workspace, WorkspaceAuth};
use crate::config::{credentials_path, ensure_parent_dir};
use crate::error::{AuthError, ConfigError, Result};
use std::fs;

/// Load credentials from file, hydrating secrets from macOS Keychain
pub fn load_credentials() -> Result<Credentials> {
    let path = credentials_path()?;

    if !path.exists() {
        return Ok(Credentials::new());
    }

    let contents = fs::read_to_string(&path)
        .map_err(|e| ConfigError::ReadError(e.to_string()))?;

    let mut creds: Credentials = serde_json::from_str(&contents)
        .map_err(|e| ConfigError::ParseError(e.to_string()))?;

    // Hydrate tokens from Keychain
    for workspace in &mut creds.workspaces {
        match &mut workspace.auth {
            WorkspaceAuth::Standard { token } => {
                if token == KEYCHAIN_PLACEHOLDER {
                    let account = keychain_account(&workspace.workspace_url, "token");
                    if let Some(real_token) = keychain_get(&account, KEYCHAIN_SERVICE) {
                        *token = real_token;
                    }
                }
            }
            WorkspaceAuth::Browser {
                xoxc_token,
                xoxd_cookie,
            } => {
                if xoxc_token == KEYCHAIN_PLACEHOLDER {
                    let account = keychain_account(&workspace.workspace_url, "xoxc_token");
                    if let Some(real_token) = keychain_get(&account, KEYCHAIN_SERVICE) {
                        *xoxc_token = real_token;
                    }
                }
                if xoxd_cookie == KEYCHAIN_PLACEHOLDER {
                    let account = keychain_account(&workspace.workspace_url, "xoxd_cookie");
                    if let Some(real_cookie) = keychain_get(&account, KEYCHAIN_SERVICE) {
                        *xoxd_cookie = real_cookie;
                    }
                }
            }
        }
    }

    Ok(creds)
}

/// Save credentials to file, dehydrating secrets to macOS Keychain
pub fn save_credentials(creds: &Credentials) -> Result<()> {
    let path = credentials_path()?;
    ensure_parent_dir(&path)?;

    // Clone and dehydrate tokens to Keychain
    let mut creds_to_save = creds.clone();
    creds_to_save.updated_at = Some(chrono::Utc::now().to_rfc3339());

    for workspace in &mut creds_to_save.workspaces {
        match &mut workspace.auth {
            WorkspaceAuth::Standard { token } => {
                let account = keychain_account(&workspace.workspace_url, "token");
                if keychain_set(&account, token, KEYCHAIN_SERVICE) {
                    *token = KEYCHAIN_PLACEHOLDER.to_string();
                }
            }
            WorkspaceAuth::Browser {
                xoxc_token,
                xoxd_cookie,
            } => {
                let xoxc_account = keychain_account(&workspace.workspace_url, "xoxc_token");
                if keychain_set(&xoxc_account, xoxc_token, KEYCHAIN_SERVICE) {
                    *xoxc_token = KEYCHAIN_PLACEHOLDER.to_string();
                }

                let xoxd_account = keychain_account(&workspace.workspace_url, "xoxd_cookie");
                if keychain_set(&xoxd_account, xoxd_cookie, KEYCHAIN_SERVICE) {
                    *xoxd_cookie = KEYCHAIN_PLACEHOLDER.to_string();
                }
            }
        }
    }

    let json = serde_json::to_string_pretty(&creds_to_save)
        .map_err(|e| ConfigError::WriteError(e.to_string()))?;

    fs::write(&path, json)
        .map_err(|e| ConfigError::WriteError(e.to_string()))?;

    Ok(())
}

/// Upsert a single workspace (update if exists by URL, otherwise append)
pub fn upsert_workspace(workspace: Workspace) -> Result<()> {
    let mut creds = load_credentials()?;

    let normalized = normalize_workspace_url(&workspace.workspace_url)
        .map_err(|e| ConfigError::ParseError(e))?;

    // Find existing workspace by normalized URL
    if let Some(existing) = creds
        .workspaces
        .iter_mut()
        .find(|w| normalize_workspace_url(&w.workspace_url).ok() == Some(normalized.clone()))
    {
        *existing = workspace;
    } else {
        creds.workspaces.push(workspace);
    }

    save_credentials(&creds)
}

/// Upsert multiple workspaces
pub fn upsert_workspaces(workspaces: Vec<Workspace>) -> Result<()> {
    for workspace in workspaces {
        upsert_workspace(workspace)?;
    }
    Ok(())
}

/// Set the default workspace URL
pub fn set_default_workspace(workspace_url: &str) -> Result<()> {
    let mut creds = load_credentials()?;
    let normalized = normalize_workspace_url(workspace_url)
        .map_err(|e| ConfigError::ParseError(e))?;

    // Verify workspace exists
    if !creds
        .workspaces
        .iter()
        .any(|w| normalize_workspace_url(&w.workspace_url).ok() == Some(normalized.clone()))
    {
        return Err(AuthError::WorkspaceNotFound(workspace_url.to_string()).into());
    }

    creds.default_workspace_url = Some(normalized);
    save_credentials(&creds)
}

/// Remove a workspace from config
pub fn remove_workspace(workspace_url: &str) -> Result<()> {
    let mut creds = load_credentials()?;
    let normalized = normalize_workspace_url(workspace_url)
        .map_err(|e| ConfigError::ParseError(e))?;

    // Remove workspace
    creds
        .workspaces
        .retain(|w| normalize_workspace_url(&w.workspace_url).ok() != Some(normalized.clone()));

    // Clear default if it was the removed workspace
    if creds.default_workspace_url.as_ref() == Some(&normalized) {
        creds.default_workspace_url = None;
    }

    save_credentials(&creds)
}

/// Resolve a workspace by URL
pub fn resolve_workspace_for_url(workspace_url: &str) -> Result<Workspace> {
    let creds = load_credentials()?;
    let normalized = normalize_workspace_url(workspace_url)
        .map_err(|e| ConfigError::ParseError(e))?;

    creds
        .workspaces
        .into_iter()
        .find(|w| normalize_workspace_url(&w.workspace_url).ok() == Some(normalized.clone()))
        .ok_or_else(|| AuthError::WorkspaceNotFound(workspace_url.to_string()).into())
}

/// Resolve the default workspace
pub fn resolve_default_workspace() -> Result<Workspace> {
    let creds = load_credentials()?;

    if let Some(default_url) = creds.default_workspace_url {
        return resolve_workspace_for_url(&default_url);
    }

    // If only one workspace, use it
    if creds.workspaces.len() == 1 {
        return creds
            .workspaces
            .into_iter()
            .next()
            .ok_or_else(|| AuthError::NoCredentials.into());
    }

    // Multiple workspaces but no default
    if creds.workspaces.is_empty() {
        return Err(AuthError::NoCredentials.into());
    }

    Err(AuthError::NoDefaultWorkspace.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_upsert_workspace() {
        // This test would require mocking the filesystem
        // For now just verify the functions exist and types compile
        let _ = load_credentials();
    }
}
