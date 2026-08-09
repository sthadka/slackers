use crate::auth::resolve_auth;
use crate::cli::{ChannelCommand, ChannelInviteOptions, ChannelMarkOptions, ChannelMembersOptions, ChannelNewOptions};
use crate::error::Result;
use crate::output::to_json_output;
use crate::render::format::OutputFormat;
use crate::slack::{
    get_conversation_info, get_user, join_conversation, leave_conversation, list_conversations,
    SlackClient,
};
use serde_json::{self, json};

pub async fn handle_channel(subcommand: ChannelCommand) -> Result<()> {
    match subcommand {
        ChannelCommand::List {
            workspace,
            types,
            exclude_archived,
            limit,
            resolve_users,
            all,
            format,
        } => handle_channel_list(workspace.as_deref(), types, exclude_archived, limit, resolve_users, all, &format).await,
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
        ChannelCommand::Members(opts) => handle_channel_members(opts).await,
        ChannelCommand::New(opts) => handle_channel_new(opts).await,
        ChannelCommand::Invite(opts) => handle_channel_invite(opts).await,
        ChannelCommand::Rename { channel, name, workspace } => {
            handle_channel_rename(&channel, &name, workspace.as_deref()).await
        }
    }
}

async fn handle_channel_list(
    workspace: Option<&str>,
    types: Option<Vec<String>>,
    exclude_archived: bool,
    limit: u32,
    resolve_users: bool,
    all: bool,
    format: &str,
) -> Result<()> {
    let auth_result = resolve_auth(workspace)?;
    let client = SlackClient::new(auth_result.auth, auth_result.workspace_url);

    let member_only = !all;
    let fmt = OutputFormat::from_str(format).unwrap_or_default();
    let streaming = fmt == OutputFormat::Json && !resolve_users;

    let mut stream_page = |page: &[crate::slack::channels::CompactChannel]| {
        for ch in page {
            println!("{}", serde_json::to_string(ch).unwrap());
        }
    };

    let mut channels = list_conversations(
        &client,
        types,
        exclude_archived,
        Some(limit as usize),
        member_only,
        if streaming { Some(&mut stream_page) } else { None },
    )
    .await?;

    if streaming {
        return Ok(());
    }

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

    match fmt {
        OutputFormat::Json => println!("{}", to_json_output(&json!(channels))),
        _ => {
            let headers = ["id", "name", "type"];
            let rows: Vec<Vec<String>> = channels
                .iter()
                .map(|ch| {
                    let id = ch.id.clone();
                    let name = ch.name.clone().unwrap_or_default();
                    let kind = if ch.is_im == Some(true) { "dm" }
                        else if ch.is_mpim == Some(true) { "mpim" }
                        else if ch.is_private == Some(true) { "private" }
                        else { "public" };
                    vec![id, name, kind.to_string()]
                })
                .collect();
            println!("{}", fmt.render_rows(&headers, &rows));
        }
    }

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

    let output = if crate::output::is_quiet() {
        json!({ "ok": true })
    } else {
        json!({
            "ok": true,
            "channel": channel_info,
        })
    };
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

    let output = if crate::output::is_quiet() {
        json!({ "ok": true })
    } else {
        json!({
            "ok": true,
            "channel": channel_id,
        })
    };
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

async fn handle_channel_members(opts: ChannelMembersOptions) -> Result<()> {
    let auth_result = resolve_auth(opts.workspace.as_deref())?;
    let client = SlackClient::new(auth_result.auth, auth_result.workspace_url);

    let channel_id = crate::slack::channels::resolve_channel_id(&client, &opts.target).await?;

    let streaming = !opts.resolve_users;

    let mut stream_page = |page: &[String]| {
        for user_id in page {
            println!("{}", json!({ "user_id": user_id }));
        }
    };

    let member_ids = client
        .list_channel_members(
            &channel_id,
            None,
            if streaming { Some(&mut stream_page) } else { None },
        )
        .await?;

    if streaming {
        return Ok(());
    }

    let member_count = member_ids.len();
    let mut members: Vec<serde_json::Value> = Vec::new();
    for user_id in &member_ids {
        let mut entry = json!({ "user_id": user_id });
        if let Ok(user) = get_user(&client, user_id).await {
            let name = user
                .display_name
                .filter(|s| !s.is_empty())
                .or(user.real_name)
                .or(user.name);
            if let Some(n) = name {
                entry["name"] = json!(n);
            }
        }
        members.push(entry);
    }

    let output = json!({
        "channel_id": channel_id,
        "member_count": member_count,
        "members": members,
    });
    println!("{}", to_json_output(&output));

    Ok(())
}

async fn handle_channel_new(opts: ChannelNewOptions) -> Result<()> {
    let auth_result = resolve_auth(opts.workspace.as_deref())?;
    let client = SlackClient::new(auth_result.auth, auth_result.workspace_url);

    let channel = client.create_channel(&opts.name, opts.private).await?;

    let output = if crate::output::is_quiet() {
        json!({ "ok": true })
    } else {
        let channel_id = channel
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let name = channel
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or(&opts.name)
            .to_string();
        json!({
            "ok": true,
            "channel_id": channel_id,
            "name": name,
        })
    };
    println!("{}", to_json_output(&output));

    Ok(())
}

async fn handle_channel_invite(opts: ChannelInviteOptions) -> Result<()> {
    let auth_result = resolve_auth(opts.workspace.as_deref())?;
    let client = SlackClient::new(auth_result.auth, auth_result.workspace_url);

    // Resolve channel name → ID
    let channel_id = crate::slack::channels::resolve_channel_id(&client, &opts.target).await?;

    let invited_count = opts.users.len();
    client.invite_to_channel(&channel_id, &opts.users).await?;

    let output = if crate::output::is_quiet() {
        json!({ "ok": true })
    } else {
        json!({
            "ok": true,
            "channel_id": channel_id,
            "invited_count": invited_count,
        })
    };
    println!("{}", to_json_output(&output));

    Ok(())
}

async fn handle_channel_rename(channel: &str, new_name: &str, workspace: Option<&str>) -> Result<()> {
    let auth_result = resolve_auth(workspace)?;
    let client = SlackClient::new(auth_result.auth, auth_result.workspace_url);

    let channel_id = crate::slack::channels::resolve_channel_id(&client, channel).await?;

    let channel_info = client.rename_channel(&channel_id, new_name).await?;

    let output = if crate::output::is_quiet() {
        json!({ "ok": true })
    } else {
        let name = channel_info
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or(new_name)
            .to_string();
        json!({
            "ok": true,
            "channel_id": channel_id,
            "name": name,
        })
    };
    println!("{}", to_json_output(&output));

    Ok(())
}
