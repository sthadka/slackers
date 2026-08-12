use crate::cli::WatchCommand;
use crate::error::Result;
use crate::store::Store;
use serde_json::json;
use std::collections::HashMap;
use std::time::Duration;

pub async fn handle_watch(cmd: WatchCommand) -> Result<()> {
    let resolved = crate::auth::resolve_auth(None)?;
    let workspace_url = resolved.workspace_url.unwrap_or_default();
    let db_path = crate::config::store_db_path(&workspace_url)?;
    let store = Store::open(&db_path)?;

    // Resolve channel names to IDs
    let mut channel_ids: Vec<String> = Vec::new();
    let mut channel_names: HashMap<String, String> = HashMap::new();

    for ch_input in &cmd.channels {
        let name = ch_input.trim_start_matches('#');
        if name.starts_with('C') || name.starts_with('G') {
            // Looks like a channel ID
            channel_ids.push(name.to_string());
            // Try to look up name from store
            if let Ok(Some(ch)) = store.get_channel_by_id(name) {
                if let Some(n) = ch.name {
                    channel_names.insert(name.to_string(), n);
                }
            }
        } else {
            // It's a channel name; resolve via store
            match store.get_channel_by_name(name) {
                Ok(Some(ch)) => {
                    channel_ids.push(ch.id.clone());
                    channel_names.insert(ch.id, name.to_string());
                }
                _ => {
                    return Err(crate::error::SlackersError::Store(format!(
                        "Channel '{}' not found in local store. Run sync first.",
                        ch_input
                    )));
                }
            }
        }
    }

    if channel_ids.is_empty() {
        return Err(crate::error::SlackersError::Store(
            "No channels specified.".to_string(),
        ));
    }

    // Build the link regex once if --has-link is set
    let link_re = if cmd.has_link {
        Some(regex::Regex::new(r"https?://").unwrap())
    } else {
        None
    };

    // Track the last seen ts per channel, starting from current time
    let now_ts = format!(
        "{}.000000",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    );
    let mut last_ts: HashMap<String, String> = HashMap::new();
    for id in &channel_ids {
        last_ts.insert(id.clone(), now_ts.clone());
    }

    let is_json = cmd.format == "json";

    // Set up Ctrl+C handler
    let shutdown = tokio::signal::ctrl_c();
    tokio::pin!(shutdown);

    loop {
        tokio::select! {
            _ = &mut shutdown => {
                break;
            }
            _ = tokio::time::sleep(Duration::from_millis(500)) => {
                for ch_id in &channel_ids {
                    let since = last_ts.get(ch_id).cloned().unwrap_or_else(|| now_ts.clone());
                    let messages = store.list_messages(ch_id, Some(&since), None, 100)?;

                    for msg in &messages {
                        // Skip the exact timestamp we already saw
                        if msg.ts <= since {
                            continue;
                        }

                        // Apply --user filter
                        if let Some(ref user_filter) = cmd.user {
                            if msg.user_id.as_deref() != Some(user_filter.as_str()) {
                                continue;
                            }
                        }

                        // Apply --has-link filter
                        if let Some(ref re) = link_re {
                            let text = msg.text.as_deref().unwrap_or("");
                            let rendered = msg.rendered.as_deref().unwrap_or("");
                            if !re.is_match(text) && !re.is_match(rendered) {
                                continue;
                            }
                        }

                        let ch_name = channel_names.get(ch_id).cloned();

                        if is_json {
                            let out = json!({
                                "channel_id": ch_id,
                                "channel_name": ch_name,
                                "ts": msg.ts,
                                "user_id": msg.user_id,
                                "text": msg.text,
                            });
                            println!("{}", serde_json::to_string(&out).unwrap_or_default());
                        } else {
                            let name_display = ch_name
                                .as_deref()
                                .unwrap_or(ch_id.as_str());
                            let user_display = msg
                                .user_id
                                .as_deref()
                                .unwrap_or("unknown");
                            let text_display = msg
                                .rendered
                                .as_deref()
                                .or(msg.text.as_deref())
                                .unwrap_or("");
                            println!(
                                "[{}] [#{}] @{}: {}",
                                msg.ts, name_display, user_display, text_display
                            );
                        }

                        // Update last seen ts
                        if let Some(last) = last_ts.get_mut(ch_id) {
                            if msg.ts > *last {
                                *last = msg.ts.clone();
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
