use crate::auth::resolve_auth;
use crate::cli::UserCommand;
use crate::error::Result;
use crate::output::to_json_output;
use crate::slack::{get_user, list_users, SlackClient};
use serde_json::json;

pub async fn handle_user(subcommand: UserCommand) -> Result<()> {
    match subcommand {
        UserCommand::List {
            workspace,
            limit,
            cursor: _,
            include_bots,
        } => handle_user_list(workspace.as_deref(), limit, include_bots).await,
        UserCommand::Get { user, workspace } => {
            handle_user_get(&user, workspace.as_deref()).await
        }
    }
}

async fn handle_user_list(
    workspace: Option<&str>,
    limit: u32,
    include_bots: bool,
) -> Result<()> {
    // Resolve auth
    let auth_result = resolve_auth(workspace)?;
    let client = SlackClient::new(auth_result.auth, auth_result.workspace_url);

    // List users
    let users = list_users(&client, Some(limit as usize), include_bots).await?;

    // Output as JSON array
    let output = json!(users);
    println!("{}", to_json_output(&output));

    Ok(())
}

async fn handle_user_get(identifier: &str, workspace: Option<&str>) -> Result<()> {
    // Resolve auth
    let auth_result = resolve_auth(workspace)?;
    let client = SlackClient::new(auth_result.auth, auth_result.workspace_url);

    // Get user
    let user = get_user(&client, identifier).await?;

    // Output as JSON object
    let output = json!(user);
    println!("{}", to_json_output(&output));

    Ok(())
}
