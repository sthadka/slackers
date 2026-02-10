use crate::auth::resolve_auth;
use crate::cli::{MessageCommand, MessageGetOptions, MessageListOptions, ReactCommand};
use crate::error::{Result, SlackersError};
use crate::output::to_json_output;
use crate::slack::{
    download_file, fetch_message, fetch_thread, get_thread_summary, to_compact_message,
    CompactMessageOptions, SlackClient,
};
use crate::target::{parse_msg_target, MsgTarget};
use serde_json::json;

pub async fn handle_message(subcommand: MessageCommand) -> Result<()> {
    match subcommand {
        MessageCommand::Get { target, options } => handle_message_get(&target, options).await,
        MessageCommand::List { target, options } => handle_message_list(&target, options).await,
        MessageCommand::Send {
            target,
            text,
            workspace,
            thread_ts,
        } => handle_message_send(&target, &text, workspace.as_deref(), thread_ts.as_deref()).await,
        MessageCommand::React { subcommand } => handle_react(subcommand).await,
    }
}

async fn handle_message_get(target: &str, options: MessageGetOptions) -> Result<()> {
    // Parse target
    let msg_target = parse_msg_target(target)?;

    let (channel_id, message_ts, workspace_url, thread_ts_hint) = match msg_target {
        MsgTarget::Url(msg_ref) => (
            msg_ref.channel_id.clone(),
            msg_ref.message_ts.clone(),
            Some(msg_ref.workspace_url.clone()),
            msg_ref.thread_ts_hint.clone(),
        ),
        MsgTarget::Channel(ch) => {
            let ts = options
                .ts
                .ok_or_else(|| SlackersError::Other("--ts required for channel targets".to_string()))?;
            (ch, ts, options.workspace.clone(), options.thread_ts.clone())
        }
    };

    // Resolve auth
    let auth_result = resolve_auth(workspace_url.as_deref())?;
    let client = SlackClient::new(auth_result.auth.clone(), auth_result.workspace_url.clone());

    // Build message ref for fetching
    let msg_ref = crate::target::SlackMessageRef {
        workspace_url: workspace_url.unwrap_or_default(),
        channel_id: channel_id.clone(),
        message_ts: message_ts.clone(),
        thread_ts_hint,
        raw: target.to_string(),
        possibly_truncated: false,
    };

    // Fetch message
    let message = fetch_message(&client, &msg_ref).await?;

    // Get thread summary if this is a thread root
    let thread_summary = if let Some(thread_ts) = message.get("thread_ts").and_then(|v| v.as_str()) {
        if thread_ts == message.get("ts").and_then(|v| v.as_str()).unwrap_or("") {
            // This is a thread root
            let (_, reply_count) = get_thread_summary(&client, &channel_id, thread_ts).await?;
            Some(json!({
                "reply_count": reply_count,
                "thread_ts": thread_ts
            }))
        } else {
            None
        }
    } else {
        None
    };

    // Convert to compact message
    let max_chars = if options.max_body_chars < 0 {
        None
    } else {
        Some(options.max_body_chars as usize)
    };

    let compact_options = CompactMessageOptions {
        max_content_chars: max_chars,
        include_thread_ts: true,
    };

    let compact = to_compact_message(&message, &compact_options);

    // Download files if any
    let mut downloaded_files = Vec::new();
    if let Some(files) = message.get("files").and_then(|v| v.as_array()) {
        for file in files {
            match download_file(&client, file, &auth_result.auth).await {
                Ok(path) => {
                    downloaded_files.push(json!({
                        "id": file.get("id").and_then(|v| v.as_str()).unwrap_or(""),
                        "name": file.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                        "local_path": path.to_string_lossy()
                    }));
                }
                Err(e) => {
                    eprintln!("Warning: Failed to download file: {}", e);
                }
            }
        }
    }

    // Build output
    let mut output = serde_json::to_value(compact)?;

    if let Some(thread_info) = thread_summary {
        if let Some(obj) = output.as_object_mut() {
            obj.insert("thread_summary".to_string(), thread_info);
        }
    }

    if !downloaded_files.is_empty() {
        if let Some(obj) = output.as_object_mut() {
            obj.insert("downloaded_files".to_string(), json!(downloaded_files));
        }
    }

    println!("{}", to_json_output(&output));
    Ok(())
}

async fn handle_message_list(target: &str, options: MessageListOptions) -> Result<()> {
    // Parse target
    let msg_target = parse_msg_target(target)?;

    let (channel_id, thread_ts, workspace_url) = match msg_target {
        MsgTarget::Url(msg_ref) => {
            let thread = if let Some(ts) = msg_ref.thread_ts_hint.as_ref() {
                ts.clone()
            } else {
                msg_ref.message_ts.clone()
            };
            (msg_ref.channel_id.clone(), thread, Some(msg_ref.workspace_url.clone()))
        }
        MsgTarget::Channel(ch) => {
            // Try --thread-ts first, then --ts
            let ts = options
                .thread_ts
                .or(options.ts)
                .ok_or_else(|| {
                    SlackersError::Other("--thread-ts or --ts required for channel targets".to_string())
                })?;
            (ch, ts, options.workspace.clone())
        }
    };

    // Resolve auth
    let auth_result = resolve_auth(workspace_url.as_deref())?;
    let client = SlackClient::new(auth_result.auth.clone(), auth_result.workspace_url.clone());

    // Fetch thread
    let messages = fetch_thread(&client, &channel_id, &thread_ts).await?;

    // Convert to compact messages
    let max_chars = if options.max_body_chars < 0 {
        None
    } else {
        Some(options.max_body_chars as usize)
    };

    let compact_options = CompactMessageOptions {
        max_content_chars: max_chars,
        include_thread_ts: false, // Don't include thread_ts in each message (redundant)
    };

    let mut compact_messages = Vec::new();
    for msg in &messages {
        let compact = to_compact_message(msg, &compact_options);
        compact_messages.push(serde_json::to_value(compact)?);
    }

    // Download files from all messages
    let mut all_downloaded_files = Vec::new();
    for msg in &messages {
        if let Some(files) = msg.get("files").and_then(|v| v.as_array()) {
            for file in files {
                match download_file(&client, file, &auth_result.auth).await {
                    Ok(path) => {
                        all_downloaded_files.push(json!({
                            "id": file.get("id").and_then(|v| v.as_str()).unwrap_or(""),
                            "name": file.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                            "local_path": path.to_string_lossy()
                        }));
                    }
                    Err(e) => {
                        eprintln!("Warning: Failed to download file: {}", e);
                    }
                }
            }
        }
    }

    // Build output
    let mut output = json!({
        "channel_id": channel_id,
        "thread_ts": thread_ts,
        "messages": compact_messages
    });

    if !all_downloaded_files.is_empty() {
        if let Some(obj) = output.as_object_mut() {
            obj.insert("downloaded_files".to_string(), json!(all_downloaded_files));
        }
    }

    println!("{}", to_json_output(&output));
    Ok(())
}

async fn handle_message_send(
    target: &str,
    text: &str,
    workspace: Option<&str>,
    thread_ts: Option<&str>,
) -> Result<()> {
    // Parse target
    let msg_target = parse_msg_target(target)?;

    let (channel_id, workspace_url, auto_thread_ts) = match msg_target {
        MsgTarget::Url(msg_ref) => {
            // URL targets auto-thread to the message
            let auto_ts = if thread_ts.is_none() {
                Some(msg_ref.message_ts.clone())
            } else {
                None
            };
            (msg_ref.channel_id.clone(), Some(msg_ref.workspace_url.clone()), auto_ts)
        }
        MsgTarget::Channel(ch) => (ch, workspace.map(|s| s.to_string()), None),
    };

    // Resolve auth
    let auth_result = resolve_auth(workspace_url.as_deref())?;
    let client = SlackClient::new(auth_result.auth, auth_result.workspace_url);

    // Build params
    let mut params = vec![
        ("channel".to_string(), channel_id),
        ("text".to_string(), text.to_string()),
    ];

    // Add thread_ts if provided or auto-detected
    if let Some(ts) = thread_ts.or(auto_thread_ts.as_deref()) {
        params.push(("thread_ts".to_string(), ts.to_string()));
    }

    // Send message
    let response = client.api_call("chat.postMessage", params).await?;

    // Extract and output result
    let output = json!({
        "ok": true,
        "ts": response.get("ts"),
        "channel": response.get("channel")
    });

    println!("{}", to_json_output(&output));
    Ok(())
}

async fn handle_react(subcommand: ReactCommand) -> Result<()> {
    match subcommand {
        ReactCommand::Add {
            target,
            emoji,
            workspace,
            ts,
        } => handle_react_add(&target, &emoji, workspace.as_deref(), ts.as_deref()).await,
        ReactCommand::Remove {
            target,
            emoji,
            workspace,
            ts,
        } => handle_react_remove(&target, &emoji, workspace.as_deref(), ts.as_deref()).await,
    }
}

async fn handle_react_add(
    target: &str,
    emoji: &str,
    workspace: Option<&str>,
    ts: Option<&str>,
) -> Result<()> {
    react_operation(target, emoji, workspace, ts, "reactions.add").await
}

async fn handle_react_remove(
    target: &str,
    emoji: &str,
    workspace: Option<&str>,
    ts: Option<&str>,
) -> Result<()> {
    react_operation(target, emoji, workspace, ts, "reactions.remove").await
}

async fn react_operation(
    target: &str,
    emoji: &str,
    workspace: Option<&str>,
    ts: Option<&str>,
    api_method: &str,
) -> Result<()> {
    // Parse target
    let msg_target = parse_msg_target(target)?;

    let (channel_id, message_ts, workspace_url) = match msg_target {
        MsgTarget::Url(msg_ref) => (
            msg_ref.channel_id.clone(),
            msg_ref.message_ts.clone(),
            Some(msg_ref.workspace_url.clone()),
        ),
        MsgTarget::Channel(ch) => {
            let ts_val = ts
                .ok_or_else(|| SlackersError::Other("--ts required for channel targets".to_string()))?;
            (ch, ts_val.to_string(), workspace.map(|s| s.to_string()))
        }
    };

    // Normalize emoji name
    let normalized_emoji = crate::slack::emoji::normalize_reaction_name(emoji);

    // Resolve auth
    let auth_result = resolve_auth(workspace_url.as_deref())?;
    let client = SlackClient::new(auth_result.auth, auth_result.workspace_url);

    // Call API
    let params = vec![
        ("channel".to_string(), channel_id),
        ("timestamp".to_string(), message_ts),
        ("name".to_string(), normalized_emoji),
    ];

    client.api_call(api_method, params).await?;

    println!("{}", to_json_output(&json!({ "ok": true })));
    Ok(())
}
