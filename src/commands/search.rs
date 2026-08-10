use crate::auth::resolve_auth;
use crate::cli::SearchCommand;
use crate::error::Result;
use crate::output::to_json_output;
use crate::render::format::OutputFormat;
use crate::slack::{
    search_slack, AdvancedQueryFilters, ContentType, SearchKind, SearchOptions,
    SlackClient, SortOrder,
};
use crate::slack::user_cache::{collect_referenced_user_ids, resolve_users_by_id, to_referenced_users};
use crate::slack::users::CompactSlackUser;
use serde_json::{json, Value};
use std::collections::HashMap;

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

fn build_name_map(users_by_id: &HashMap<String, CompactSlackUser>) -> HashMap<String, String> {
    users_by_id
        .iter()
        .map(|(uid, user)| {
            let name = user.display_name.as_ref().filter(|s| !s.is_empty())
                .or(user.real_name.as_ref())
                .or(user.name.as_ref())
                .cloned()
                .unwrap_or_else(|| uid.clone());
            (uid.clone(), name)
        })
        .collect()
}

async fn handle_search_impl(
    query: &str,
    kind: SearchKind,
    options: crate::cli::SearchOptions,
) -> Result<()> {
    // Resolve auth
    let auth_result = resolve_auth(options.workspace.as_deref())?;
    let client = SlackClient::new(auth_result.auth.clone(), auth_result.workspace_url.clone());

    let content_type = match options.content_type {
        Some(crate::cli::ContentTypeArg::Text) => ContentType::Text,
        Some(crate::cli::ContentTypeArg::Image) => ContentType::Image,
        Some(crate::cli::ContentTypeArg::Snippet) => ContentType::Snippet,
        Some(crate::cli::ContentTypeArg::File) => ContentType::File,
        Some(crate::cli::ContentTypeArg::Any) | None => ContentType::Any,
    };

    let sort = match options.sort {
        Some(crate::cli::SortArg::Relevance) => SortOrder::Relevance,
        Some(crate::cli::SortArg::Timestamp) | None => SortOrder::Timestamp,
    };

    // Parse advanced filters
    let advanced_filters = AdvancedQueryFilters {
        has_link: options.has_link,
        has_emoji: options.has_emoji,
        from_me: options.from_me,
    };

    // Build search options
    let max_content_chars = if options.max_body_chars < 0 {
        usize::MAX
    } else {
        options.max_body_chars as usize
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

    let mut msgs: Vec<Value> = result
        .messages
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .map(|m| serde_json::to_value(m).unwrap_or(Value::Null))
        .collect();

    let mut referenced_users_value: Option<Value> = None;
    if options.resolve_users && !msgs.is_empty() {
        let user_ids = collect_referenced_user_ids(&msgs);
        let users_by_id = resolve_users_by_id(
            &client,
            auth_result.workspace_url.as_deref(),
            &user_ids,
            options.refresh_users,
        ).await;
        let name_map = build_name_map(&users_by_id);
        for msg in &mut msgs {
            if let Some(obj) = msg.as_object_mut() {
                if let Some(uid) = obj.get("user").and_then(|v| v.as_str()).map(|s| s.to_string()) {
                    if let Some(name) = name_map.get(&uid) {
                        obj.insert("user_name".to_string(), json!(name));
                    }
                }
            }
        }
        referenced_users_value = to_referenced_users(&user_ids, &users_by_id).map(|m| json!(m));
    }
    let messages_value = json!(msgs);

    // Determine output format
    let fmt = OutputFormat::from_str(options.format.as_str()).unwrap_or_default();

    // Build output
    let mut output = json!({
        "messages": messages_value,
        "files": result.files,
    });

    if let Some(ref_users) = referenced_users_value {
        if let Some(obj) = output.as_object_mut() {
            obj.insert("referenced_users".to_string(), ref_users);
        }
    }

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
