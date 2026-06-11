use crate::auth::resolve_auth;
use crate::cli::{ChannelCommand, ChannelMarkOptions};
use crate::error::Result;
use crate::output::to_json_output;
use crate::slack::{
    get_conversation_info, get_user, join_conversation, leave_conversation, list_conversations,
    SlackClient,
};
use serde_json::json;

pub async fn handle_channel(subcommand: ChannelCommand) -> Result<()> {
    match subcommand {
        ChannelCommand::List {
            workspace,
            types,
            exclude_archived,
            limit,
            resolve_users,
        } => handle_channel_list(workspace.as_deref(), types, exclude_archived, limit, resolve_users).await,
        ChannelCommand::Get {
            channel,
            workspace,
            include_num_members,
        } => {
            handle_channel_get(&channel, workspace.as_deref(), include_num_members).await
        }
        ChannelCommand::Join { channel, workspace } => {
            handle_channel_join(&channel, workspace.as_deref()).await
        }
        ChannelCommand::Leave { channel, workspace } => {
            handle_channel_leave(&channel, workspace.as_deref()).await
        }
        ChannelCommand::Mark(opts) => handle_channel_mark(opts).await,
    }
}

async fn handle_channel_list(
    workspace: Option<&str>,
    types: Option<Vec<String>>,
    exclude_archived: bool,
    limit: u32,
    resolve_users: bool,
) -> Result<()> {
    let auth_result = resolve_auth(workspace)?;
    let client = SlackClient::new(auth_result.auth, auth_result.workspace_url);

    let mut channels = list_conversations(
        &client,
        types,
        exclude_archived,
        Some(limit as usize),
    )
    .await?;

    if resolve_users {
        for ch in &mut channels {
            if ch.is_im == Some(true) {
                if let Some(ref uid) = ch.user.clone() {
                    if let Ok(u) = get_user(&client, uid).await {
                        ch.user_name = u.display_name
                            .filter(|s| !s.is_empty())
                            .or(u.real_name)
                            .or(u.name);
                    }
                }
            }
        }
    }

    let output = json!(channels);
    println!("{}", to_json_output(&output));

    Ok(())
}

async fn handle_channel_get(
    channel: &str,
    workspace: Option<&str>,
    include_num_members: bool,
) -> Result<()> {
    // Resolve auth
    let auth_result = resolve_auth(workspace)?;
    let client = SlackClient::new(auth_result.auth, auth_result.workspace_url);

    // conversations.info ONLY accepts channel IDs, not names
    // Resolve channel name to ID if needed (handles both IDs and names)
    let channel_id = crate::slack::channels::resolve_channel_id(&client, channel).await?;

    // Get channel info
    let channel_info = get_conversation_info(&client, &channel_id, include_num_members).await?;

    // Output as JSON object
    let output = json!(channel_info);
    println!("{}", to_json_output(&output));

    Ok(())
}

async fn handle_channel_join(channel: &str, workspace: Option<&str>) -> Result<()> {
    // Resolve auth
    let auth_result = resolve_auth(workspace)?;
    let client = SlackClient::new(auth_result.auth, auth_result.workspace_url);

    // Join conversation (API handles both IDs and names)
    let channel_info = join_conversation(&client, channel).await?;

    // Output as JSON object
    let output = json!({
        "ok": true,
        "channel": channel_info,
    });
    println!("{}", to_json_output(&output));

    Ok(())
}

async fn handle_channel_leave(channel: &str, workspace: Option<&str>) -> Result<()> {
    // Resolve auth
    let auth_result = resolve_auth(workspace)?;
    let client = SlackClient::new(auth_result.auth, auth_result.workspace_url);

    // Resolve channel name to ID if needed
    let channel_id = crate::slack::channels::resolve_channel_id(&client, channel).await?;

    // Leave conversation
    leave_conversation(&client, &channel_id).await?;

    // Output success
    let output = json!({
        "ok": true,
        "channel": channel_id,
    });
    println!("{}", to_json_output(&output));

    Ok(())
}

async fn handle_channel_mark(opts: ChannelMarkOptions) -> Result<()> {
    // Resolve auth
    let auth_result = resolve_auth(opts.workspace.as_deref())?;
    let client = SlackClient::new(auth_result.auth, auth_result.workspace_url);

    // Resolve channel name/id to a canonical channel ID
    let channel_id = crate::slack::channels::resolve_channel_id(&client, &opts.target).await?;

    // Mark the channel as read up to the given ts
    client.mark_channel(&channel_id, &opts.ts).await?;

    let output = json!({ "ok": true });
    println!("{}", to_json_output(&output));

    Ok(())
}
