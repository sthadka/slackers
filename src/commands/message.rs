use crate::app_config::{load_app_config, load_history_cursors, save_history_cursors};
use crate::auth::resolve_auth;
use crate::cli::{MessageCommand, MessageDeleteOptions, MessageGetOptions, MessageHistoryOptions, MessageListOptions, MessagePinOptions, MessageUnpinOptions, MessageUpdateOptions, ReactCommand};
use std::collections::{HashMap, HashSet};
use crate::error::{Result, SlackersError};
use crate::output::to_json_output;
use crate::render::format::OutputFormat;
use crate::slack::{
    download_file, fetch_message, fetch_thread, filter_messages, format_outbound_slack_text,
    get_thread_summary, get_user, resolve_channel_id, to_compact_message, CompactMessageOptions,
    MessageFilter, SlackClient,
};
use crate::target::{parse_msg_target, MsgTarget};
use chrono::NaiveDate;
use indicatif::{ProgressBar, ProgressStyle};
use serde_json::{json, Value};
use std::time::Duration;

pub async fn handle_message(subcommand: MessageCommand) -> Result<()> {
    match subcommand {
        MessageCommand::Get { target, options } => handle_message_get(&target, options).await,
        MessageCommand::List { target, options } => handle_message_list(&target, options).await,
        MessageCommand::Send {
            target,
            text,
            workspace,
            thread_ts,
            reply_broadcast,
            blocks,
        } => handle_message_send(&target, &text, workspace.as_deref(), thread_ts.as_deref(), reply_broadcast, blocks.as_deref()).await,
        MessageCommand::React { subcommand } => handle_react(subcommand).await,
        MessageCommand::History { channel, options } => {
            handle_message_history(&channel, options).await
        }
        MessageCommand::ThreadParticipants {
            target,
            channel,
            ts,
            workspace,
            resolve_users,
        } => {
            handle_thread_participants(
                target.as_deref(),
                channel.as_deref(),
                ts.as_deref(),
                workspace.as_deref(),
                resolve_users,
            )
            .await
        }
        MessageCommand::Pin(opts) => handle_message_pin(opts).await,
        MessageCommand::Unpin(opts) => handle_message_unpin(opts).await,
        MessageCommand::Delete(opts) => handle_message_delete(opts).await,
        MessageCommand::Update(opts) => handle_message_update(opts).await,
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

    // Resolve user IDs to display names if requested
    if options.resolve_users {
        let msgs_slice = std::slice::from_ref(&output);
        let user_map = build_user_map(&client, msgs_slice).await;
        enrich_message_with_user_name(&mut output, &user_map);
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
    let mut messages = fetch_thread(&client, &channel_id, &thread_ts).await?;

    // Apply filters if specified
    let filter = MessageFilter {
        user: options.user.clone(),
        has_link: options.has_link,
        has_file: options.has_file,
        has_reaction: options.has_reaction,
    };

    messages = filter_messages(messages, &filter);

    // Apply --with-reaction filter (client-side)
    if let Some(ref reaction_name) = options.with_reaction {
        let name = reaction_name.trim_matches(':');
        messages.retain(|msg| {
            msg.get("reactions")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().any(|r| r.get("name").and_then(|n| n.as_str()) == Some(name)))
                .unwrap_or(false)
        });
    }

    // Apply --without-reaction filter (client-side)
    if let Some(ref reaction_name) = options.without_reaction {
        let name = reaction_name.trim_matches(':');
        messages.retain(|msg| {
            !msg.get("reactions")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().any(|r| r.get("name").and_then(|n| n.as_str()) == Some(name)))
                .unwrap_or(false)
        });
    }

    // Apply limit if specified
    if let Some(limit) = options.limit {
        messages.truncate(limit);
    }

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

    // Resolve user IDs to display names if requested
    if options.resolve_users {
        let user_map = build_user_map(&client, &compact_messages).await;
        for msg in &mut compact_messages {
            enrich_message_with_user_name(msg, &user_map);
        }
    }

    // Determine output format
    let fmt = OutputFormat::from_str(&options.format).unwrap_or_default();

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

    match fmt {
        OutputFormat::Json => println!("{}", to_json_output(&output)),
        _ => {
            // For non-JSON formats, render the messages list as a table
            let msgs = compact_messages;
            let headers = ["ts", "user", "text"];
            let rows: Vec<Vec<String>> = msgs
                .iter()
                .map(|m| {
                    vec![
                        m.get("ts").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        m.get("user").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        m.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    ]
                })
                .collect();
            println!("{}", fmt.render_rows(&headers, &rows));
        }
    }
    Ok(())
}

fn load_blocks_from_path(path: &str) -> Result<Value> {
    let raw = if path == "-" {
        use std::io::Read;
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| SlackersError::Other(format!("--blocks: failed to read stdin: {}", e)))?;
        buf
    } else {
        std::fs::read_to_string(path)
            .map_err(|e| SlackersError::Other(format!("--blocks: failed to read {}: {}", path, e)))?
    };

    let parsed: Value = serde_json::from_str(&raw)
        .map_err(|e| {
            let source = if path == "-" { "stdin" } else { path };
            SlackersError::Other(format!("--blocks: failed to parse JSON from {}: {}", source, e))
        })?;

    let arr = parsed.as_array().ok_or_else(|| {
        SlackersError::Other(format!(
            "--blocks: expected a JSON array of Block Kit blocks, got {}",
            value_type_name(&parsed)
        ))
    })?;

    for (i, el) in arr.iter().enumerate() {
        if !el.is_object() {
            return Err(SlackersError::Other(format!(
                "--blocks: element at index {} is not a Block Kit block object (got {})",
                i,
                value_type_name(el)
            )));
        }
    }

    Ok(parsed)
}

fn value_type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

async fn handle_message_send(
    target: &str,
    text: &str,
    workspace: Option<&str>,
    thread_ts: Option<&str>,
    reply_broadcast: bool,
    blocks_path: Option<&str>,
) -> Result<()> {
    // Load blocks from file/stdin if provided
    let blocks = blocks_path.map(load_blocks_from_path).transpose()?;

    // Parse target
    let msg_target = parse_msg_target(target)?;

    let (channel_id, workspace_url, auto_thread_ts) = match msg_target {
        MsgTarget::Url(msg_ref) => {
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
    let formatted_text = format_outbound_slack_text(text);
    let mut params = vec![
        ("channel".to_string(), channel_id),
        ("text".to_string(), formatted_text),
    ];

    if let Some(ref blocks_val) = blocks {
        params.push(("blocks".to_string(), serde_json::to_string(blocks_val).unwrap()));
    }

    // Add thread_ts if provided or auto-detected
    if let Some(ts) = thread_ts.or(auto_thread_ts.as_deref()) {
        params.push(("thread_ts".to_string(), ts.to_string()));
    }

    // Add reply_broadcast if requested
    if reply_broadcast {
        params.push(("reply_broadcast".to_string(), "true".to_string()));
    }

    // Send message
    let response = client.api_call("chat.postMessage", params).await?;

    let ts = response.get("ts").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let channel_id = response.get("channel").and_then(|v| v.as_str()).unwrap_or("").to_string();

    // Fetch permalink via chat.getPermalink
    let permalink = if !ts.is_empty() && !channel_id.is_empty() {
        let permalink_params = vec![
            ("channel".to_string(), channel_id.clone()),
            ("message_ts".to_string(), ts.clone()),
        ];
        match client.api_call("chat.getPermalink", permalink_params).await {
            Ok(pl_resp) => pl_resp
                .get("permalink")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            Err(_) => None,
        }
    } else {
        None
    };

    // Extract and output result
    let output = json!({
        "ok": true,
        "ts": ts,
        "channel": channel_id,
        "permalink": permalink
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

async fn handle_message_history(channel: &str, options: MessageHistoryOptions) -> Result<()> {
    let config = load_app_config();

    let auth_result = resolve_auth(options.workspace.as_deref())?;
    let client = SlackClient::new(auth_result.auth.clone(), auth_result.workspace_url.clone());

    let channel_id = resolve_channel_id(&client, channel).await?;

    // Determine output file: explicit --output or <channel>-history.json in CWD.
    let output_path = {
        let default_name = format!("{}-history.json", channel.trim_start_matches('#'));
        std::path::PathBuf::from(options.output.as_deref().unwrap_or(&default_name))
    };

    // Load messages already saved from a previous (possibly interrupted) run.
    let mut output_messages: Vec<Value> = load_history_file(&output_path);
    if !output_messages.is_empty() {
        eprintln!(
            "[slackers] loaded {} existing messages from {}",
            output_messages.len(),
            output_path.display()
        );
    }

    // Derive resume ts from the file; fall back to cursor store.
    let mut cursors = load_history_cursors();
    let resume_ts = if options.before.is_none() {
        output_messages
            .last()
            .and_then(|m| m.get("ts"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| {
                if config.history.auto_resume {
                    cursors.get(&channel_id).cloned()
                } else {
                    None
                }
            })
    } else {
        None
    };
    if let Some(ref ts) = resume_ts {
        eprintln!(
            "[slackers] resuming — fetching messages older than {}",
            ts
        );
    }

    let oldest = options.after.as_deref().map(date_to_ts).transpose()?;
    let latest = match resume_ts {
        Some(ts) => Some(ts),
        None => options.before.as_deref().map(date_to_ts).transpose()?,
    };

    let max_chars = if options.max_body_chars < 0 {
        None
    } else {
        Some(options.max_body_chars as usize)
    };
    let compact_options = CompactMessageOptions {
        max_content_chars: max_chars,
        include_thread_ts: true,
    };

    // ── Phase 1: fetch all history pages ─────────────────────────────────────
    // The file is written after each page so it exists on disk even if
    // Phase 2 (thread fetching) is interrupted.
    let spinner = new_spinner("Fetching channel history...");

    let remaining_limit = options.limit.saturating_sub(output_messages.len());
    let mut raw_messages: Vec<Value> = Vec::new();
    let mut page_cursor: Option<String> = None;

    loop {
        if raw_messages.len() >= remaining_limit {
            break;
        }

        let mut params = vec![
            ("channel".to_string(), channel_id.clone()),
            ("limit".to_string(), "200".to_string()),
        ];
        if let Some(ref ts) = oldest {
            params.push(("oldest".to_string(), ts.clone()));
        }
        if let Some(ref ts) = latest {
            params.push(("latest".to_string(), ts.clone()));
        }
        if let Some(ref c) = page_cursor {
            params.push(("cursor".to_string(), c.clone()));
        }

        let response = client.api_call("conversations.history", params).await?;

        let mut page: Vec<Value> = response
            .get("messages")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().cloned().collect())
            .unwrap_or_default();

        let can_take = remaining_limit.saturating_sub(raw_messages.len());
        if page.len() > can_take {
            page.truncate(can_take);
        }

        raw_messages.extend(page);
        spinner.set_message(format!("Fetching channel history… {} messages", raw_messages.len()));

        // Write the file after each page (no threads yet) so it exists on disk immediately.
        let partial: Vec<Value> = raw_messages
            .iter()
            .map(|m| serde_json::to_value(to_compact_message(m, &compact_options)))
            .collect::<std::result::Result<_, _>>()?;
        let mut combined = output_messages.clone();
        combined.extend(partial);
        write_history_file(&output_path, channel, &channel_id, &combined)?;

        page_cursor = response
            .get("response_metadata")
            .and_then(|m| m.get("next_cursor"))
            .and_then(|c| c.as_str())
            .filter(|c| !c.is_empty())
            .map(|c| c.to_string());

        if page_cursor.is_none() {
            break;
        }

        // Stay under Slack Tier-3 limit (50 req/min) between history pages.
        tokio::time::sleep(Duration::from_millis(1200)).await;
    }

    let raw_messages = apply_config_filters(raw_messages, &config.history);
    spinner.finish_with_message(format!(
        "Fetched {} new messages from history",
        raw_messages.len()
    ));

    // ── Phase 2: compact + thread fetch + incremental file write ─────────────
    let bar = new_progress_bar(raw_messages.len() as u64, "Processing");

    for msg in &raw_messages {
        let compact = to_compact_message(msg, &compact_options);
        let reply_count = compact.reply_count.unwrap_or(0);
        let ts = compact.ts.clone();
        let mut msg_value = serde_json::to_value(compact)?;

        if options.include_threads && reply_count > 0 {
            bar.set_message(format!("fetching thread {}", ts));
            // 2 s between conversations.replies calls — comfortably under Tier-3 (50 req/min).
            tokio::time::sleep(Duration::from_millis(2000)).await;
            let thread = fetch_thread(&client, &channel_id, &ts).await?;
            let replies: Vec<Value> = thread
                .iter()
                .skip(1)
                .map(|r| serde_json::to_value(to_compact_message(r, &compact_options)))
                .collect::<std::result::Result<_, _>>()?;
            if let Some(obj) = msg_value.as_object_mut() {
                obj.insert("thread".to_string(), json!(replies));
            }
        }

        output_messages.push(msg_value);

        // Persist after every message so an interrupt loses at most one.
        write_history_file(&output_path, channel, &channel_id, &output_messages)?;

        // Keep cursor store in sync.
        if let Some(oldest_ts) = output_messages
            .last()
            .and_then(|m| m.get("ts"))
            .and_then(|v| v.as_str())
        {
            cursors.insert(channel_id.clone(), oldest_ts.to_string());
            save_history_cursors(&cursors);
        }

        bar.set_message(String::new());
        bar.inc(1);
    }

    bar.finish_with_message(format!(
        "done — {} messages in {}",
        output_messages.len(),
        output_path.display()
    ));

    // Clear cursor on clean completion.
    cursors.remove(&channel_id);
    save_history_cursors(&cursors);

    let resume_before = output_messages
        .last()
        .and_then(|m| m.get("ts"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let mut output = json!({
        "channel": channel,
        "channel_id": channel_id,
        "message_count": output_messages.len(),
        "output_file": output_path.display().to_string(),
        "messages": output_messages,
    });
    if let Some(ts) = resume_before {
        output["resume_before"] = json!(ts);
    }

    println!("{}", to_json_output(&output));
    Ok(())
}

fn new_spinner(msg: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.cyan} {msg}")
            .unwrap(),
    );
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(Duration::from_millis(120));
    pb
}

fn new_progress_bar(total: u64, label: &str) -> ProgressBar {
    let pb = ProgressBar::new(total);
    pb.set_style(
        ProgressStyle::default_bar()
            .template(&format!(
                "{{spinner:.cyan}} {} [{{bar:40.cyan/blue}}] {{pos}}/{{len}} {{msg}} ({{elapsed}})",
                label
            ))
            .unwrap()
            .progress_chars("=> "),
    );
    pb.enable_steady_tick(Duration::from_millis(120));
    pb
}

/// Load previously saved processed messages from a history file.
/// Returns an empty vec if the file does not exist or cannot be parsed.
fn load_history_file(path: &std::path::Path) -> Vec<Value> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let parsed: Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    parsed
        .get("messages")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
}

/// Write the current set of processed messages to the history file.
fn write_history_file(
    path: &std::path::Path,
    channel: &str,
    channel_id: &str,
    messages: &[Value],
) -> Result<()> {
    let content = serde_json::to_string_pretty(&json!({
        "channel": channel,
        "channel_id": channel_id,
        "message_count": messages.len(),
        "messages": messages,
    }))?;
    std::fs::write(path, content)?;
    Ok(())
}

fn apply_config_filters(
    messages: Vec<Value>,
    config: &crate::app_config::HistoryConfig,
) -> Vec<Value> {
    if config.exclude_subtypes.is_empty() && config.exclude_users.is_empty() {
        return messages;
    }
    messages
        .into_iter()
        .filter(|msg| {
            // Drop messages whose subtype is in exclude_subtypes.
            if !config.exclude_subtypes.is_empty() {
                if let Some(subtype) = msg.get("subtype").and_then(|v| v.as_str()) {
                    if config.exclude_subtypes.iter().any(|s| s == subtype) {
                        return false;
                    }
                }
            }
            // Drop messages from excluded users.
            if !config.exclude_users.is_empty() {
                if let Some(user) = msg.get("user").and_then(|v| v.as_str()) {
                    if config.exclude_users.iter().any(|u| u == user) {
                        return false;
                    }
                }
            }
            true
        })
        .collect()
}

async fn handle_thread_participants(
    target: Option<&str>,
    channel_arg: Option<&str>,
    ts_arg: Option<&str>,
    workspace: Option<&str>,
    resolve_users: bool,
) -> Result<()> {
    // Resolve channel_id and thread_ts from either a URL target or explicit flags
    let (channel_id, thread_ts, workspace_url) = if let Some(url) = target {
        let msg_target = parse_msg_target(url)?;
        match msg_target {
            MsgTarget::Url(msg_ref) => {
                let ts = msg_ref
                    .thread_ts_hint
                    .clone()
                    .unwrap_or_else(|| msg_ref.message_ts.clone());
                (msg_ref.channel_id.clone(), ts, Some(msg_ref.workspace_url.clone()))
            }
            MsgTarget::Channel(ch) => {
                let ts = ts_arg
                    .ok_or_else(|| SlackersError::Other("--ts required when target is a channel ID".to_string()))?
                    .to_string();
                (ch, ts, workspace.map(|s| s.to_string()))
            }
        }
    } else {
        let ch = channel_arg
            .ok_or_else(|| SlackersError::Other("--channel required when no URL target is given".to_string()))?
            .to_string();
        let ts = ts_arg
            .ok_or_else(|| SlackersError::Other("--ts required when no URL target is given".to_string()))?
            .to_string();
        (ch, ts, workspace.map(|s| s.to_string()))
    };

    // Resolve auth
    let auth_result = resolve_auth(workspace_url.as_deref())?;
    let client = SlackClient::new(auth_result.auth, auth_result.workspace_url);

    // Fetch thread messages
    let messages = fetch_thread(&client, &channel_id, &thread_ts).await?;

    // Count messages per user
    let mut counts: HashMap<String, u32> = HashMap::new();
    for msg in &messages {
        if let Some(user) = msg.get("user").and_then(|v| v.as_str()) {
            *counts.entry(user.to_string()).or_insert(0) += 1;
        }
    }

    // Build participant list sorted by message count descending
    let mut participants: Vec<(String, u32)> = counts.into_iter().collect();
    participants.sort_by(|a, b| b.1.cmp(&a.1));

    // Optionally resolve user names
    let mut output_participants: Vec<Value> = Vec::new();
    for (user_id, count) in participants {
        let mut entry = json!({
            "user_id": user_id,
            "message_count": count
        });
        if resolve_users {
            match crate::slack::get_user(&client, &user_id).await {
                Ok(user) => {
                    let display = user
                        .display_name
                        .or(user.real_name)
                        .or(user.name)
                        .unwrap_or_else(|| user_id.clone());
                    if let Some(obj) = entry.as_object_mut() {
                        obj.insert("name".to_string(), json!(display));
                    }
                }
                Err(_) => {} // Best-effort; skip if lookup fails
            }
        }
        output_participants.push(entry);
    }

    let output = json!({
        "channel_id": channel_id,
        "thread_ts": thread_ts,
        "participants": output_participants
    });

    println!("{}", to_json_output(&output));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{load_blocks_from_path, value_type_name};
    use serde_json::json;
    use crate::render::format::{Formattable, OutputFormat};

    /// Demonstrate that any serde::Serialize value (including message types)
    /// can be formatted as Table or JSON via the Formattable blanket impl.
    #[test]
    fn test_message_formattable_table_vs_json() {
        let msg = json!({
            "ts": "1700000000.000001",
            "user": "U123",
            "text": "Hello world"
        });

        let json_out = msg.format(&OutputFormat::Json);
        assert!(json_out.contains("\"ts\""), "JSON output should contain field 'ts'");
        assert!(json_out.contains("Hello world"), "JSON output should contain message text");

        let table_out = msg.format(&OutputFormat::Table);
        assert!(table_out.contains("ts"), "Table output should contain key 'ts'");
        assert!(table_out.contains("Hello world"), "Table output should contain message text");

        // Table and JSON must differ in shape
        assert_ne!(json_out, table_out, "Table and JSON outputs should differ");
    }

    fn make_msg_with_reactions(ts: &str, reactions: &[&str]) -> serde_json::Value {
        let reaction_objs: Vec<serde_json::Value> = reactions
            .iter()
            .map(|name| json!({"name": name, "count": 1}))
            .collect();
        json!({"ts": ts, "user": "U1", "text": "hi", "reactions": reaction_objs})
    }

    fn make_msg_no_reactions(ts: &str) -> serde_json::Value {
        json!({"ts": ts, "user": "U1", "text": "hi"})
    }

    #[test]
    fn test_with_reaction_filter() {
        let msgs = vec![
            make_msg_with_reactions("1.0", &["thumbsup", "rocket"]),
            make_msg_with_reactions("2.0", &["rocket"]),
            make_msg_no_reactions("3.0"),
        ];
        let name = "thumbsup";
        let filtered: Vec<_> = msgs
            .into_iter()
            .filter(|msg| {
                msg.get("reactions")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().any(|r| r.get("name").and_then(|n| n.as_str()) == Some(name)))
                    .unwrap_or(false)
            })
            .collect();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0]["ts"], "1.0");
    }

    #[test]
    fn test_without_reaction_filter() {
        let msgs = vec![
            make_msg_with_reactions("1.0", &["thumbsup"]),
            make_msg_with_reactions("2.0", &["rocket"]),
            make_msg_no_reactions("3.0"),
        ];
        let name = "thumbsup";
        let filtered: Vec<_> = msgs
            .into_iter()
            .filter(|msg| {
                !msg.get("reactions")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().any(|r| r.get("name").and_then(|n| n.as_str()) == Some(name)))
                    .unwrap_or(false)
            })
            .collect();
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0]["ts"], "2.0");
        assert_eq!(filtered[1]["ts"], "3.0");
    }

    #[test]
    fn test_load_blocks_valid_array() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_blocks_valid.json");
        std::fs::write(&path, r#"[{"type":"section","text":{"type":"mrkdwn","text":"Hello"}}]"#).unwrap();
        let result = load_blocks_from_path(path.to_str().unwrap());
        assert!(result.is_ok());
        let blocks = result.unwrap();
        assert!(blocks.is_array());
        assert_eq!(blocks.as_array().unwrap().len(), 1);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_load_blocks_non_array_json() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_blocks_non_array.json");
        std::fs::write(&path, r#"{"type":"section"}"#).unwrap();
        let result = load_blocks_from_path(path.to_str().unwrap());
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("expected a JSON array"), "Error: {}", err);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_load_blocks_malformed_json() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_blocks_malformed.json");
        std::fs::write(&path, "not json at all").unwrap();
        let result = load_blocks_from_path(path.to_str().unwrap());
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("failed to parse JSON"), "Error: {}", err);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_load_blocks_non_object_element() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_blocks_non_object.json");
        std::fs::write(&path, r#"[{"type":"section"}, "not_an_object"]"#).unwrap();
        let result = load_blocks_from_path(path.to_str().unwrap());
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("element at index 1"), "Error: {}", err);
        assert!(err.contains("not a Block Kit block object"), "Error: {}", err);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_load_blocks_null_element() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_blocks_null_elem.json");
        std::fs::write(&path, r#"[null]"#).unwrap();
        let result = load_blocks_from_path(path.to_str().unwrap());
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("element at index 0"), "Error: {}", err);
        assert!(err.contains("null"), "Error: {}", err);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_load_blocks_file_not_found() {
        let result = load_blocks_from_path("/nonexistent/path/blocks.json");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("failed to read"), "Error: {}", err);
    }

    #[test]
    fn test_value_type_name() {
        assert_eq!(value_type_name(&json!(null)), "null");
        assert_eq!(value_type_name(&json!(true)), "boolean");
        assert_eq!(value_type_name(&json!(42)), "number");
        assert_eq!(value_type_name(&json!("hello")), "string");
        assert_eq!(value_type_name(&json!([])), "array");
        assert_eq!(value_type_name(&json!({})), "object");
    }
}

async fn handle_message_pin(opts: MessagePinOptions) -> Result<()> {
    let auth_result = resolve_auth(opts.workspace.as_deref())?;
    let client = SlackClient::new(auth_result.auth, auth_result.workspace_url);
    client.pin_message(&opts.channel, &opts.ts).await?;
    println!("{}", to_json_output(&json!({ "ok": true })));
    Ok(())
}

async fn handle_message_unpin(opts: MessageUnpinOptions) -> Result<()> {
    let auth_result = resolve_auth(opts.workspace.as_deref())?;
    let client = SlackClient::new(auth_result.auth, auth_result.workspace_url);
    client.unpin_message(&opts.channel, &opts.ts).await?;
    println!("{}", to_json_output(&json!({ "ok": true })));
    Ok(())
}

async fn handle_message_delete(opts: MessageDeleteOptions) -> Result<()> {
    let auth_result = resolve_auth(opts.workspace.as_deref())?;
    let client = SlackClient::new(auth_result.auth, auth_result.workspace_url);
    client.delete_message(&opts.channel, &opts.ts).await?;
    println!("{}", to_json_output(&json!({ "ok": true })));
    Ok(())
}

async fn handle_message_update(opts: MessageUpdateOptions) -> Result<()> {
    let auth_result = resolve_auth(opts.workspace.as_deref())?;
    let client = SlackClient::new(auth_result.auth, auth_result.workspace_url);
    let formatted_text = format_outbound_slack_text(&opts.text);
    let resp = client.update_message(&opts.channel, &opts.ts, &formatted_text).await?;
    println!("{}", to_json_output(&json!({ "ok": true, "ts": resp.ts, "text": resp.text })));
    Ok(())
}

/// Collect all unique user IDs from a slice of compact-message JSON values,
/// resolve them to display names via `users.info`, and return a map from
/// user-ID to display name.  Look-up failures are silently skipped so the
/// rest of the output is unaffected.
async fn build_user_map(client: &SlackClient, messages: &[serde_json::Value]) -> HashMap<String, String> {
    // Collect unique user IDs
    let ids: HashSet<String> = messages
        .iter()
        .filter_map(|m| m.get("user").and_then(|v| v.as_str()).map(|s| s.to_string()))
        .collect();

    let mut map = HashMap::new();
    for uid in ids {
        if let Ok(user) = get_user(client, &uid).await {
            let name = user
                .display_name
                .filter(|s| !s.is_empty())
                .or(user.real_name)
                .or(user.name)
                .unwrap_or_else(|| uid.clone());
            map.insert(uid, name);
        }
    }
    map
}

/// Apply a user-ID → display-name map to a mutable JSON value.
/// Replaces `"user": "U..."` with `"user_name": "<display name>"` (keeps the
/// original `"user"` field intact so callers can still access the raw ID).
fn enrich_message_with_user_name(msg: &mut serde_json::Value, user_map: &HashMap<String, String>) {
    if let Some(obj) = msg.as_object_mut() {
        if let Some(uid) = obj.get("user").and_then(|v| v.as_str()).map(|s| s.to_string()) {
            if let Some(name) = user_map.get(&uid) {
                obj.insert("user_name".to_string(), serde_json::json!(name));
            }
        }
    }
}

/// Convert a YYYY-MM-DD string to a Unix timestamp string for the Slack API.
/// If the string already contains a '.', it is treated as a raw ts and passed through unchanged.
fn date_to_ts(s: &str) -> Result<String> {
    if s.contains('.') {
        return Ok(s.to_string());
    }
    let date = NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| {
        SlackersError::Other(format!(
            "Invalid date '{}': expected YYYY-MM-DD or a raw Slack ts",
            s
        ))
    })?;
    let ts = date
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| SlackersError::Other("Failed to construct datetime".to_string()))?
        .and_utc()
        .timestamp();
    Ok(ts.to_string())
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
