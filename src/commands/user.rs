use crate::auth::resolve_auth;
use crate::cli::UserCommand;
use crate::error::Result;
use crate::output::to_json_output;
use crate::render::format::OutputFormat;
use crate::slack::{get_user, list_users, SlackClient};
use crate::store::Store;
use serde_json::{self, json};

/// Try to open the local store if the `[store] enabled = true` setting is active.
/// Returns `None` when the store is disabled or cannot be opened.
fn try_open_store(workspace_url: Option<&str>) -> Option<Store> {
    let workspace_url = workspace_url?;
    crate::config::open_store_if_enabled(workspace_url).ok().flatten()
}

pub async fn handle_user(subcommand: UserCommand) -> Result<()> {
    match subcommand {
        UserCommand::List {
            workspace,
            limit,
            cursor: _,
            include_bots,
            format,
        } => handle_user_list(workspace.as_deref(), limit, include_bots, format.as_str()).await,
        UserCommand::Get { user, workspace } => {
            handle_user_get(&user, workspace.as_deref()).await
        }
    }
}

async fn handle_user_list(
    workspace: Option<&str>,
    limit: u32,
    include_bots: bool,
    format: &str,
) -> Result<()> {
    let auth_result = resolve_auth(workspace)?;

    let fmt = OutputFormat::from_str(format).unwrap_or_default();

    // Try local store first
    if let Some(store) = try_open_store(auth_result.workspace_url.as_deref()) {
        if let Ok(store_users) = store.list_users() {
            if !store_users.is_empty() {
                let users: Vec<_> = store_users
                    .into_iter()
                    .filter(|u| include_bots || u.is_bot != Some(true))
                    .take(limit as usize)
                    .collect();

                return render_user_list(&fmt, &users);
            }
        }
    }

    // Fall back to API
    let client = SlackClient::new(auth_result.auth, auth_result.workspace_url);
    let streaming = fmt == OutputFormat::Json;

    let mut stream_page = |page: &[crate::slack::users::CompactSlackUser]| {
        for u in page {
            println!("{}", serde_json::to_string(u).unwrap());
        }
    };

    let users = list_users(
        &client,
        Some(limit as usize),
        include_bots,
        if streaming { Some(&mut stream_page) } else { None },
    ).await?;

    if streaming {
        return Ok(());
    }

    render_user_list(&fmt, &users)
}

fn render_user_list(
    fmt: &OutputFormat,
    users: &[crate::slack::users::CompactSlackUser],
) -> Result<()> {
    match fmt {
        OutputFormat::Json => {
            println!("{}", to_json_output(&json!(users)));
        }
        _ => {
            let headers = ["id", "name", "real_name"];
            let rows: Vec<Vec<String>> = users
                .iter()
                .map(|u| {
                    vec![
                        u.id.clone(),
                        u.name.clone().unwrap_or_default(),
                        u.real_name.clone().unwrap_or_default(),
                    ]
                })
                .collect();
            println!("{}", fmt.render_rows(&headers, &rows));
        }
    }
    Ok(())
}

async fn handle_user_get(identifier: &str, workspace: Option<&str>) -> Result<()> {
    // Resolve auth
    let auth_result = resolve_auth(workspace)?;

    // Try local store first
    if let Some(store) = try_open_store(auth_result.workspace_url.as_deref()) {
        let store_result = if identifier.starts_with('U') || identifier.starts_with('W') {
            store.get_user_by_id(identifier).ok().flatten()
        } else {
            // Try by name (strip leading @ if present)
            let name = identifier.trim_start_matches('@');
            store.get_user_by_name(name).ok().flatten()
        };
        if let Some(user) = store_result {
            let output = json!(user);
            println!("{}", to_json_output(&output));
            return Ok(());
        }
    }

    // Fall back to API
    let client = SlackClient::new(auth_result.auth, auth_result.workspace_url);

    // Get user
    let user = get_user(&client, identifier).await?;

    // Output as JSON object
    let output = json!(user);
    println!("{}", to_json_output(&output));

    Ok(())
}
