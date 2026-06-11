use crate::auth::resolve_auth;
use crate::cli::SearchCommand;
use crate::error::Result;
use crate::output::to_json_output;
use crate::render::format::OutputFormat;
use crate::slack::{
    get_user, search_slack, AdvancedQueryFilters, ContentType, SearchKind, SearchOptions,
    SlackClient, SortOrder,
};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

pub async fn handle_search(subcommand: SearchCommand) -> Result<()> {
    match subcommand {
        SearchCommand::All { query, options } => {
            handle_search_impl(&query, SearchKind::All, options).await
        }
        SearchCommand::Messages { query, options } => {
            handle_search_impl(&query, SearchKind::Messages, options).await
        }
        SearchCommand::Files { query, options } => {
            handle_search_impl(&query, SearchKind::Files, options).await
        }
    }
}

/// Collect unique user IDs from a slice of JSON message values, resolve them to
/// display names via `users.info`, and return a map from user-ID → display name.
/// Look-up failures are silently skipped.
async fn build_search_user_map(client: &SlackClient, messages: &[Value]) -> HashMap<String, String> {
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

async fn handle_search_impl(
    query: &str,
    kind: SearchKind,
    options: crate::cli::SearchOptions,
) -> Result<()> {
    // Resolve auth
    let auth_result = resolve_auth(options.workspace.as_deref())?;
    let client = SlackClient::new(auth_result.auth.clone(), auth_result.workspace_url.clone());

    // Parse content type
    let content_type = match options.content_type.as_deref() {
        Some("text") => ContentType::Text,
        Some("image") => ContentType::Image,
        Some("snippet") => ContentType::Snippet,
        Some("file") => ContentType::File,
        _ => ContentType::Any,
    };

    // Parse sort order
    let sort = match options.sort.as_deref() {
        Some("relevance") | Some("score") => SortOrder::Relevance,
        _ => SortOrder::Timestamp,
    };

    // Parse advanced filters
    let advanced_filters = AdvancedQueryFilters {
        has_link: options.has_link,
        has_emoji: options.has_emoji,
        from_me: options.from_me,
    };

    // Build search options
    let max_content_chars = if options.max_content_chars < 0 {
        usize::MAX
    } else {
        options.max_content_chars as usize
    };

    let search_opts = SearchOptions {
        workspace_url: auth_result.workspace_url.as_deref(),
        query,
        kind,
        channels: &options.channel,
        user: options.user.as_deref(),
        after: options.after.as_deref(),
        before: options.before.as_deref(),
        content_type,
        limit: options.limit as usize,
        max_content_chars,
        download: true, // Always download files for local access
        sort,
        advanced_filters,
    };

    // Execute search
    let result = search_slack(&client, &auth_result.auth, search_opts).await?;

    // Optionally resolve user IDs to display names in message results
    let messages_value: Value = if options.resolve_users {
        let mut msgs: Vec<Value> = result
            .messages
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .map(|m| serde_json::to_value(m).unwrap_or(Value::Null))
            .collect();

        if !msgs.is_empty() {
            let user_map = build_search_user_map(&client, &msgs).await;
            for msg in &mut msgs {
                if let Some(obj) = msg.as_object_mut() {
                    if let Some(uid) = obj.get("user").and_then(|v| v.as_str()).map(|s| s.to_string()) {
                        if let Some(name) = user_map.get(&uid) {
                            obj.insert("user_name".to_string(), json!(name));
                        }
                    }
                }
            }
        }
        json!(msgs)
    } else {
        json!(result.messages)
    };

    // Determine output format
    let fmt = OutputFormat::from_str(&options.format).unwrap_or_default();

    // Build output
    let output = json!({
        "messages": messages_value,
        "files": result.files,
    });

    match fmt {
        OutputFormat::Json => println!("{}", to_json_output(&output)),
        _ => {
            // For non-JSON formats, render messages as a table
            let msgs: Vec<Value> = messages_value
                .as_array()
                .cloned()
                .unwrap_or_default();
            let headers = ["ts", "channel", "user", "text"];
            let rows: Vec<Vec<String>> = msgs
                .iter()
                .map(|m| {
                    vec![
                        m.get("ts").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        m.get("channel").and_then(|v| v.as_str()).unwrap_or("").to_string(),
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
