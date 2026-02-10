use crate::auth::{
    extract_from_chrome, extract_from_slack_desktop, load_credentials, normalize_workspace_url,
    parse_curl_from_stdin, remove_workspace, resolve_auth, set_default_workspace, upsert_workspaces,
    upsert_workspace, Workspace, WorkspaceAuth,
};
use crate::cli::AuthCommand;
use crate::error::Result;
use crate::output::to_json_output;
use crate::slack::SlackClient;
use crate::util::redact_secret;
use serde::Serialize;

pub async fn handle_auth(command: AuthCommand) -> Result<()> {
    match command {
        AuthCommand::Whoami => handle_whoami().await,
        AuthCommand::Test { workspace } => handle_test(workspace).await,
        AuthCommand::Add {
            workspace_url,
            token,
            xoxc,
            xoxd,
        } => handle_add(workspace_url, token, xoxc, xoxd).await,
        AuthCommand::SetDefault { workspace_url } => handle_set_default(workspace_url).await,
        AuthCommand::Remove { workspace_url } => handle_remove(workspace_url).await,
        AuthCommand::ImportDesktop => handle_import_desktop().await,
        AuthCommand::ImportChrome => handle_import_chrome().await,
        AuthCommand::ParseCurl => handle_parse_curl().await,
    }
}

async fn handle_whoami() -> Result<()> {
    let creds = load_credentials()?;

    #[derive(Serialize)]
    struct SafeWorkspace {
        workspace_url: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        workspace_name: Option<String>,
        auth_type: String,
        token: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        cookie_d: Option<String>,
    }

    #[derive(Serialize)]
    struct SafeCredentials {
        version: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        updated_at: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        default_workspace_url: Option<String>,
        workspaces: Vec<SafeWorkspace>,
    }

    let safe = SafeCredentials {
        version: creds.version,
        updated_at: creds.updated_at,
        default_workspace_url: creds.default_workspace_url,
        workspaces: creds
            .workspaces
            .into_iter()
            .map(|w| match w.auth {
                WorkspaceAuth::Standard { token } => SafeWorkspace {
                    workspace_url: w.workspace_url,
                    workspace_name: w.workspace_name,
                    auth_type: "standard".to_string(),
                    token: redact_secret(&token, 6, 4),
                    cookie_d: None,
                },
                WorkspaceAuth::Browser {
                    xoxc_token,
                    xoxd_cookie,
                } => SafeWorkspace {
                    workspace_url: w.workspace_url,
                    workspace_name: w.workspace_name,
                    auth_type: "browser".to_string(),
                    token: redact_secret(&xoxc_token, 6, 4),
                    cookie_d: Some(redact_secret(&xoxd_cookie, 6, 4)),
                },
            })
            .collect(),
    };

    println!("{}", to_json_output(&safe));
    Ok(())
}

async fn handle_test(workspace: Option<String>) -> Result<()> {
    let resolved = resolve_auth(workspace.as_deref())?;
    let client = SlackClient::new(resolved.auth, resolved.workspace_url);

    let response = client.api_call("auth.test", vec![]).await?;
    println!("{}", to_json_output(&response));
    Ok(())
}

async fn handle_add(
    workspace_url: String,
    token: Option<String>,
    xoxc: Option<String>,
    xoxd: Option<String>,
) -> Result<()> {
    let normalized = normalize_workspace_url(&workspace_url)
        .map_err(|e| format!("Invalid workspace URL: {}", e))?;

    let auth = if let Some(token) = token {
        // Validate standard token
        if !token.starts_with("xoxb-") && !token.starts_with("xoxp-") {
            eprintln!("Warning: Token doesn't start with xoxb- or xoxp-");
        }
        WorkspaceAuth::Standard { token }
    } else if let (Some(xoxc), Some(xoxd)) = (xoxc, xoxd) {
        // Validate browser tokens
        if !xoxc.starts_with("xoxc-") {
            eprintln!("Warning: xoxc token doesn't start with xoxc-");
        }
        if !xoxd.starts_with("xoxd-") {
            eprintln!("Warning: xoxd cookie doesn't start with xoxd-");
        }
        WorkspaceAuth::Browser {
            xoxc_token: xoxc,
            xoxd_cookie: xoxd,
        }
    } else {
        return Err("Provide either --token or both --xoxc and --xoxd".into());
    };

    let workspace = Workspace {
        workspace_url: normalized.clone(),
        workspace_name: None,
        team_id: None,
        team_domain: None,
        auth,
    };

    upsert_workspace(workspace)?;

    #[derive(Serialize)]
    struct AddResult {
        message: String,
        workspace_url: String,
    }

    let result = AddResult {
        message: "Credentials saved".to_string(),
        workspace_url: normalized,
    };

    println!("{}", to_json_output(&result));
    Ok(())
}

async fn handle_set_default(workspace_url: String) -> Result<()> {
    let normalized = normalize_workspace_url(&workspace_url)
        .map_err(|e| format!("Invalid workspace URL: {}", e))?;

    set_default_workspace(&normalized)?;

    #[derive(Serialize)]
    struct SetDefaultResult {
        message: String,
        default_workspace_url: String,
    }

    let result = SetDefaultResult {
        message: "Default workspace updated".to_string(),
        default_workspace_url: normalized,
    };

    println!("{}", to_json_output(&result));
    Ok(())
}

async fn handle_remove(workspace_url: String) -> Result<()> {
    let normalized = normalize_workspace_url(&workspace_url)
        .map_err(|e| format!("Invalid workspace URL: {}", e))?;

    remove_workspace(&normalized)?;

    #[derive(Serialize)]
    struct RemoveResult {
        message: String,
        removed: String,
    }

    let result = RemoveResult {
        message: "Workspace removed".to_string(),
        removed: normalized,
    };

    println!("{}", to_json_output(&result));
    Ok(())
}

async fn handle_import_desktop() -> Result<()> {
    let extracted = extract_from_slack_desktop().await?;

    // Convert teams to workspaces
    let workspaces: Vec<Workspace> = extracted
        .teams
        .into_iter()
        .map(|team| {
            let workspace_url = normalize_workspace_url(&team.url).unwrap_or(team.url.clone());
            Workspace {
                workspace_url,
                workspace_name: team.name,
                team_id: None,
                team_domain: None,
                auth: WorkspaceAuth::Browser {
                    xoxc_token: team.token,
                    xoxd_cookie: extracted.cookie_d.clone(),
                },
            }
        })
        .collect();

    let count = workspaces.len();
    upsert_workspaces(workspaces)?;

    #[derive(Serialize)]
    struct ImportResult {
        message: String,
        count: usize,
        source: String,
    }

    let result = ImportResult {
        message: format!("Imported {} workspace(s) from Slack Desktop", count),
        count,
        source: "desktop".to_string(),
    };

    println!("{}", to_json_output(&result));
    Ok(())
}

async fn handle_import_chrome() -> Result<()> {
    let extracted = extract_from_chrome()?;

    if extracted.is_none() {
        return Err("No Slack workspaces found in Chrome. Make sure Chrome is open with Slack tabs.".into());
    }

    let extracted = extracted.unwrap();

    // Convert teams to workspaces
    let workspaces: Vec<Workspace> = extracted
        .teams
        .into_iter()
        .map(|team| {
            let workspace_url = normalize_workspace_url(&team.url).unwrap_or(team.url.clone());
            Workspace {
                workspace_url,
                workspace_name: team.name,
                team_id: None,
                team_domain: None,
                auth: WorkspaceAuth::Browser {
                    xoxc_token: team.token,
                    xoxd_cookie: extracted.cookie_d.clone(),
                },
            }
        })
        .collect();

    let count = workspaces.len();
    upsert_workspaces(workspaces)?;

    #[derive(Serialize)]
    struct ImportResult {
        message: String,
        count: usize,
        source: String,
    }

    let result = ImportResult {
        message: format!("Imported {} workspace(s) from Chrome", count),
        count,
        source: "chrome".to_string(),
    };

    println!("{}", to_json_output(&result));
    Ok(())
}

async fn handle_parse_curl() -> Result<()> {
    let workspace = parse_curl_from_stdin()?;

    upsert_workspace(workspace.clone())?;

    #[derive(Serialize)]
    struct ParseCurlResult {
        message: String,
        workspace_url: String,
    }

    let result = ParseCurlResult {
        message: "Credentials imported from cURL command".to_string(),
        workspace_url: workspace.workspace_url,
    };

    println!("{}", to_json_output(&result));
    Ok(())
}
