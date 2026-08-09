use crate::auth::resolve_auth;
use crate::cli::UserCommand;
use crate::error::Result;
use crate::output::to_json_output;
use crate::render::format::OutputFormat;
use crate::slack::{get_user, list_users, SlackClient};
use serde_json::{self, json};

pub async fn handle_user(subcommand: UserCommand) -> Result<()> {
    match subcommand {
        UserCommand::List {
            workspace,
            limit,
            cursor: _,
            include_bots,
            format,
        } => handle_user_list(workspace.as_deref(), limit, include_bots, &format).await,
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
    let client = SlackClient::new(auth_result.auth, auth_result.workspace_url);

    let fmt = OutputFormat::from_str(format).unwrap_or_default();
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
