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
use crate::store::Store;
use serde_json::{json, Value};
use std::collections::HashMap;

/// Try to open the local store if the `[store] enabled = true` setting is active.
/// Returns `None` when the store is disabled or cannot be opened.
fn try_open_store(workspace_url: Option<&str>) -> Option<Store> {
    let workspace_url = workspace_url?;
    crate::config::open_store_if_enabled(workspace_url).ok().flatten()
}

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

/// Check if we can serve a message search from the local store.
/// Returns true when the search is message-only, has no advanced filters or date
/// restrictions that the store cannot handle, and all requested channels (if any)
/// are subscribed in the store.
fn can_use_local_search(
    kind: &SearchKind,
    options: &crate::cli::SearchOptions,
    store: &Store,
) -> bool {
    // Only message searches can use FTS5 (no file search support)
    if *kind == SearchKind::Files {
        return false;
    }

    // Advanced filters (has:link, has:emoji, from:me) are not supported by FTS5
    if options.has_link || options.has_emoji || options.from_me {
        return false;
    }

    // Date-range filters are not supported by FTS5
    if options.after.is_some() || options.before.is_some() {
        return false;
    }

    // User filter is not supported by FTS5
    if options.user.is_some() {
        return false;
    }

    // --all-channels means search everything in the store (no channel filter needed)
    if options.all_channels {
        return true;
    }

    // If channels are specified, all of them must be subscribed
    if !options.channel.is_empty() {
        // We need to resolve channel names to IDs, but we only have the store.
        // For simplicity, check that all channels (by ID or name) can be found
        // and are subscribed.
        for ch in &options.channel {
            let channel_id = if ch.starts_with('C') || ch.starts_with('G') {
                Some(ch.clone())
            } else {
                let name = ch.trim_start_matches('#');
                store
                    .get_channel_by_name(name)
                    .ok()
                    .flatten()
                    .map(|c| c.id)
            };
            match channel_id {
                Some(id) => {
                    if !store.is_subscribed(&id).unwrap_or(false) {
                        return false;
                    }
                }
                None => return false,
            }
        }
    }

    true
}

/// Convert FTS5 SearchResult items to the same JSON Value format used by
/// the API search output (CompactSlackMessage-like objects with a `channel` field).
fn fts_results_to_values(results: &[crate::store::fts::SearchResult]) -> Vec<Value> {
    results
        .iter()
        .map(|r| {
            let text = r.rendered.as_deref().or(r.text.as_deref()).unwrap_or("");
            let mut obj = json!({
                "ts": r.ts,
                "text": text,
                "channel": r.channel_id,
            });
            if let Some(ref uid) = r.user_id {
                obj["user"] = json!(uid);
            }
            if let Some(ref tts) = r.thread_ts {
                obj["thread_ts"] = json!(tts);
            }
            obj
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

    // Try local store for message searches
    let local_result = if kind == SearchKind::Messages || kind == SearchKind::All {
        if let Some(store) = try_open_store(auth_result.workspace_url.as_deref()) {
            if can_use_local_search(&kind, &options, &store) {
                // When --all-channels is set, don't filter by channel
                let channel_id = if options.all_channels {
                    None
                } else if options.channel.len() == 1 {
                    let ch = &options.channel[0];
                    if ch.starts_with('C') || ch.starts_with('G') {
                        Some(ch.clone())
                    } else {
                        let name = ch.trim_start_matches('#');
                        store
                            .get_channel_by_name(name)
                            .ok()
                            .flatten()
                            .map(|c| c.id)
                    }
                } else {
                    None
                };

                let search_result = if options.highlight {
                    store.search_with_highlight(
                        query,
                        channel_id.as_deref(),
                        options.limit,
                        "**",
                        "**",
                    )
                } else {
                    store.search_messages(
                        query,
                        channel_id.as_deref(),
                        options.limit,
                    )
                };

                match search_result {
                    Ok(mut results) => {
                        // Apply --regex post-filter if set
                        if let Some(ref pattern) = options.regex {
                            if let Ok(re) = regex::Regex::new(pattern) {
                                results.retain(|r| {
                                    let text = r.rendered.as_deref()
                                        .or(r.text.as_deref())
                                        .unwrap_or("");
                                    re.is_match(text)
                                });
                            }
                        }
                        Some(results)
                    }
                    Err(_) => None, // Fall back to API on FTS5 errors
                }
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    // Determine output format
    let fmt = OutputFormat::from_str(options.format.as_str()).unwrap_or_default();

    if let Some(fts_results) = local_result {
        if kind == SearchKind::Messages {
            // Pure message search served from local store
            let msgs = fts_results_to_values(&fts_results);
            let messages_value = json!(msgs);

            let output = json!({
                "messages": messages_value,
                "source": "local",
            });

            match fmt {
                OutputFormat::Json => println!("{}", to_json_output(&output)),
                _ => {
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
            return Ok(());
        }
        // For SearchKind::All, we got local messages but still need API for files.
        // Fall through to the API path but inject local messages.
    }

    // Fall back to (or combine with) API
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
