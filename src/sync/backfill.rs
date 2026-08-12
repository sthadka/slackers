use crate::app_config::StoreConfig;
use crate::error::Result;
use crate::slack::SlackClient;
use crate::store::Store;
use indicatif::{ProgressBar, ProgressStyle};
use serde::Serialize;
use serde_json::Value;
use std::time::{Duration, Instant};

/// Statistics from a backfill or incremental sync run for a single channel.
#[derive(Debug, Clone, Serialize)]
pub struct BackfillStats {
    pub channel_id: String,
    pub messages_added: u64,
    pub messages_updated: u64,
    pub pages_fetched: u64,
    pub duration_ms: u64,
}

/// Page through `conversations.history` for a single channel, storing every
/// message (and its reactions) into the local SQLite store. Updates
/// `sync_state` after each page so an interrupted backfill can resume.
///
/// When `sync_threads` is enabled in the subscription, thread replies are
/// also fetched for messages with `reply_count > 0`.
pub async fn backfill_channel(
    client: &SlackClient,
    store: &Store,
    channel_id: &str,
    config: &StoreConfig,
) -> Result<BackfillStats> {
    let start = Instant::now();
    let mut stats = BackfillStats {
        channel_id: channel_id.to_string(),
        messages_added: 0,
        messages_updated: 0,
        pages_fetched: 0,
        duration_ms: 0,
    };

    // Determine if threads should be synced for this channel.
    let sync_threads = store
        .list_subscriptions()?
        .iter()
        .find(|s| s.channel_id == channel_id)
        .map(|s| s.sync_threads)
        .unwrap_or(config.defaults.sync_threads);

    // Check existing sync state — resume from cursor if one exists.
    let existing_state = store.get_sync_state(channel_id)?;
    let mut page_cursor: Option<String> = existing_state
        .as_ref()
        .and_then(|s| s.cursor.clone());

    // Compute retention boundary if configured.
    let retention_boundary = config.defaults.retention_days.map(|days| {
        let secs = chrono::Utc::now().timestamp() - (days as i64 * 86400);
        format!("{}.000000", secs)
    });

    let channel_label = channel_id.to_string();
    let spinner = new_spinner(&format!("Backfilling {}...", channel_label));

    // Track the newest and oldest timestamps we encounter.
    let mut newest_ts: Option<String> = existing_state
        .as_ref()
        .and_then(|s| s.newest_ts.clone());
    let mut oldest_ts: Option<String> = existing_state
        .as_ref()
        .and_then(|s| s.oldest_ts.clone());

    loop {
        let mut params = vec![
            ("channel".to_string(), channel_id.to_string()),
            ("limit".to_string(), "200".to_string()),
        ];
        if let Some(ref c) = page_cursor {
            params.push(("cursor".to_string(), c.clone()));
        }

        let response = client.api_call("conversations.history", params).await?;

        let messages: Vec<Value> = response
            .get("messages")
            .and_then(|v| v.as_array())
            .map(|arr| arr.to_vec())
            .unwrap_or_default();

        if messages.is_empty() {
            break;
        }

        stats.pages_fetched += 1;

        for msg in &messages {
            let ts = msg.get("ts").and_then(|v| v.as_str()).unwrap_or("");
            if ts.is_empty() {
                continue;
            }

            // Check retention boundary — stop if we have reached it.
            if let Some(ref boundary) = retention_boundary {
                if ts < boundary.as_str() {
                    // We have reached the retention boundary; mark complete.
                    store.update_sync_state(
                        channel_id,
                        oldest_ts.as_deref(),
                        newest_ts.as_deref(),
                        None,
                        true,
                    )?;
                    stats.duration_ms = start.elapsed().as_millis() as u64;
                    spinner.finish_with_message(format!(
                        "Backfill {} complete (retention boundary) - {} msgs, {} pages",
                        channel_label, stats.messages_added, stats.pages_fetched
                    ));
                    return Ok(stats);
                }
            }

            // Track newest / oldest timestamps.
            match newest_ts {
                None => newest_ts = Some(ts.to_string()),
                Some(ref current) if ts > current.as_str() => {
                    newest_ts = Some(ts.to_string());
                }
                _ => {}
            }
            match oldest_ts {
                None => oldest_ts = Some(ts.to_string()),
                Some(ref current) if ts < current.as_str() => {
                    oldest_ts = Some(ts.to_string());
                }
                _ => {}
            }

            // Check if message already exists to distinguish add vs update.
            let existing = store.get_message(channel_id, ts)?;
            if existing.is_some() {
                stats.messages_updated += 1;
            } else {
                stats.messages_added += 1;
            }

            // Extract fields from the API response.
            let user_id = msg.get("user").and_then(|v| v.as_str());
            let thread_ts = msg.get("thread_ts").and_then(|v| v.as_str());
            let text = msg.get("text").and_then(|v| v.as_str());
            let subtype = msg.get("subtype").and_then(|v| v.as_str());
            let reply_count = msg
                .get("reply_count")
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as i32;

            let raw_json = if config.store_raw_json {
                Some(serde_json::to_string(msg).unwrap_or_default())
            } else {
                None
            };

            store.upsert_message(
                channel_id,
                ts,
                user_id,
                thread_ts,
                text,
                None, // rendered — raw backfill does not render
                subtype,
                reply_count,
                raw_json.as_deref(),
            )?;

            // Extract and upsert reactions. The API returns:
            // "reactions": [{ "name": "thumbsup", "users": ["U1", "U2"], "count": 2 }]
            if let Some(reactions) = msg.get("reactions").and_then(|v| v.as_array()) {
                for reaction in reactions {
                    let emoji = match reaction.get("name").and_then(|v| v.as_str()) {
                        Some(e) => e,
                        None => continue,
                    };
                    if let Some(users) = reaction.get("users").and_then(|v| v.as_array()) {
                        for user_val in users {
                            if let Some(uid) = user_val.as_str() {
                                store.upsert_reaction(channel_id, ts, emoji, uid)?;
                            }
                        }
                    }
                }
            }

            // Fetch thread replies if sync_threads is enabled and message has replies.
            if sync_threads && reply_count > 0 && thread_ts.is_none() {
                // This is a thread parent (thread_ts is None means the message
                // itself is not a reply). Fetch replies.
                fetch_and_store_thread(client, store, channel_id, ts, config).await?;
            }
        }

        spinner.set_message(format!(
            "Backfilling {}... {} msgs ({} pages)",
            channel_label, stats.messages_added, stats.pages_fetched
        ));

        // Update sync_state after each page for resume capability.
        let next_cursor = response
            .get("response_metadata")
            .and_then(|m| m.get("next_cursor"))
            .and_then(|c| c.as_str())
            .filter(|c| !c.is_empty())
            .map(|c| c.to_string());

        store.update_sync_state(
            channel_id,
            oldest_ts.as_deref(),
            newest_ts.as_deref(),
            next_cursor.as_deref(),
            false,
        )?;

        page_cursor = next_cursor;

        if page_cursor.is_none() {
            // No more pages — we have reached the beginning of history.
            break;
        }

        // Rate limit: 1.2s between pages.
        tokio::time::sleep(Duration::from_millis(1200)).await;
    }

    // Mark backfill as complete (no cursor, is_complete = true).
    store.update_sync_state(
        channel_id,
        oldest_ts.as_deref(),
        newest_ts.as_deref(),
        None,
        true,
    )?;

    stats.duration_ms = start.elapsed().as_millis() as u64;
    spinner.finish_with_message(format!(
        "Backfill {} complete - {} msgs, {} pages, {:.1}s",
        channel_label,
        stats.messages_added,
        stats.pages_fetched,
        stats.duration_ms as f64 / 1000.0
    ));

    Ok(stats)
}

/// For each subscribed channel, check sync state and run `backfill_channel`.
/// Resumes interrupted backfills using the stored cursor.
pub async fn backfill_all(
    client: &SlackClient,
    store: &Store,
    config: &StoreConfig,
) -> Result<Vec<BackfillStats>> {
    let subscriptions = store.list_subscriptions()?;
    if subscriptions.is_empty() {
        eprintln!("[sync] No channel subscriptions found. Use `slackers store sub add` first.");
        return Ok(Vec::new());
    }

    let mut all_stats = Vec::new();

    for sub in &subscriptions {
        let state = store.get_sync_state(&sub.channel_id)?;

        // Skip channels that are already completely backfilled and have no
        // pending cursor (i.e. the backfill finished cleanly before).
        if let Some(ref s) = state {
            if s.is_complete && s.cursor.is_none() {
                eprintln!(
                    "[sync] Skipping {} ({}) — already complete",
                    sub.channel_name.as_deref().unwrap_or("?"),
                    sub.channel_id
                );
                continue;
            }
        }

        eprintln!(
            "[sync] Backfilling {} ({})",
            sub.channel_name.as_deref().unwrap_or("?"),
            sub.channel_id
        );

        let stats = backfill_channel(client, store, &sub.channel_id, config).await?;
        all_stats.push(stats);
    }

    Ok(all_stats)
}

/// Incremental sync: for each subscribed channel, fetch messages newer than
/// the `newest_ts` recorded in sync_state. Used by `sync once`.
pub async fn incremental_sync(
    client: &SlackClient,
    store: &Store,
    config: &StoreConfig,
) -> Result<Vec<BackfillStats>> {
    let subscriptions = store.list_subscriptions()?;
    if subscriptions.is_empty() {
        eprintln!("[sync] No channel subscriptions found. Use `slackers store sub add` first.");
        return Ok(Vec::new());
    }

    let mut all_stats = Vec::new();

    for sub in &subscriptions {
        let start = Instant::now();
        let mut stats = BackfillStats {
            channel_id: sub.channel_id.clone(),
            messages_added: 0,
            messages_updated: 0,
            pages_fetched: 0,
            duration_ms: 0,
        };

        let state = store.get_sync_state(&sub.channel_id)?;
        let oldest_bound = state.as_ref().and_then(|s| s.newest_ts.clone());

        // Determine if threads should be synced for this channel.
        let sync_threads = sub.sync_threads;

        let channel_label = sub
            .channel_name
            .as_deref()
            .unwrap_or(&sub.channel_id)
            .to_string();

        let spinner = new_spinner(&format!("Syncing {}...", channel_label));

        let mut newest_ts = oldest_bound.clone();
        let mut page_cursor: Option<String> = None;

        loop {
            let mut params = vec![
                ("channel".to_string(), sub.channel_id.clone()),
                ("limit".to_string(), "200".to_string()),
            ];
            if let Some(ref ts) = oldest_bound {
                params.push(("oldest".to_string(), ts.clone()));
            }
            if let Some(ref c) = page_cursor {
                params.push(("cursor".to_string(), c.clone()));
            }

            let response = client
                .api_call("conversations.history", params)
                .await?;

            let messages: Vec<Value> = response
                .get("messages")
                .and_then(|v| v.as_array())
                .map(|arr| arr.to_vec())
                .unwrap_or_default();

            if messages.is_empty() {
                break;
            }

            stats.pages_fetched += 1;

            for msg in &messages {
                let ts = msg.get("ts").and_then(|v| v.as_str()).unwrap_or("");
                if ts.is_empty() {
                    continue;
                }

                // Track newest ts for updating sync_state.
                match newest_ts {
                    None => newest_ts = Some(ts.to_string()),
                    Some(ref current) if ts > current.as_str() => {
                        newest_ts = Some(ts.to_string());
                    }
                    _ => {}
                }

                let existing = store.get_message(&sub.channel_id, ts)?;
                if existing.is_some() {
                    stats.messages_updated += 1;
                } else {
                    stats.messages_added += 1;
                }

                let user_id = msg.get("user").and_then(|v| v.as_str());
                let thread_ts = msg.get("thread_ts").and_then(|v| v.as_str());
                let text = msg.get("text").and_then(|v| v.as_str());
                let subtype = msg.get("subtype").and_then(|v| v.as_str());
                let reply_count = msg
                    .get("reply_count")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0) as i32;

                let raw_json = if config.store_raw_json {
                    Some(serde_json::to_string(msg).unwrap_or_default())
                } else {
                    None
                };

                store.upsert_message(
                    &sub.channel_id,
                    ts,
                    user_id,
                    thread_ts,
                    text,
                    None,
                    subtype,
                    reply_count,
                    raw_json.as_deref(),
                )?;

                // Upsert reactions.
                if let Some(reactions) = msg.get("reactions").and_then(|v| v.as_array()) {
                    for reaction in reactions {
                        let emoji = match reaction.get("name").and_then(|v| v.as_str()) {
                            Some(e) => e,
                            None => continue,
                        };
                        if let Some(users) = reaction.get("users").and_then(|v| v.as_array()) {
                            for user_val in users {
                                if let Some(uid) = user_val.as_str() {
                                    store.upsert_reaction(
                                        &sub.channel_id,
                                        ts,
                                        emoji,
                                        uid,
                                    )?;
                                }
                            }
                        }
                    }
                }

                // Fetch thread replies if needed.
                if sync_threads && reply_count > 0 && thread_ts.is_none() {
                    fetch_and_store_thread(
                        client,
                        store,
                        &sub.channel_id,
                        ts,
                        config,
                    )
                    .await?;
                }
            }

            spinner.set_message(format!(
                "Syncing {}... {} new msgs ({} pages)",
                channel_label, stats.messages_added, stats.pages_fetched
            ));

            page_cursor = response
                .get("response_metadata")
                .and_then(|m| m.get("next_cursor"))
                .and_then(|c| c.as_str())
                .filter(|c| !c.is_empty())
                .map(|c| c.to_string());

            if page_cursor.is_none() {
                break;
            }

            tokio::time::sleep(Duration::from_millis(1200)).await;
        }

        // Update sync_state with the new newest_ts.
        if let Some(ref new_newest) = newest_ts {
            store.update_sync_state(
                &sub.channel_id,
                None, // preserve existing oldest_ts
                Some(new_newest),
                None,
                state.map(|s| s.is_complete).unwrap_or(false),
            )?;
        }

        stats.duration_ms = start.elapsed().as_millis() as u64;
        spinner.finish_with_message(format!(
            "Sync {} complete - {} new, {} updated, {:.1}s",
            channel_label,
            stats.messages_added,
            stats.messages_updated,
            stats.duration_ms as f64 / 1000.0
        ));

        all_stats.push(stats);
    }

    Ok(all_stats)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Fetch all replies in a thread via `conversations.replies` and store them.
async fn fetch_and_store_thread(
    client: &SlackClient,
    store: &Store,
    channel_id: &str,
    thread_ts: &str,
    config: &StoreConfig,
) -> Result<()> {
    let mut cursor: Option<String> = None;

    loop {
        let mut params = vec![
            ("channel".to_string(), channel_id.to_string()),
            ("ts".to_string(), thread_ts.to_string()),
            ("limit".to_string(), "200".to_string()),
        ];
        if let Some(ref c) = cursor {
            params.push(("cursor".to_string(), c.clone()));
        }

        let response = client.api_call("conversations.replies", params).await?;

        let messages: Vec<Value> = response
            .get("messages")
            .and_then(|v| v.as_array())
            .map(|arr| arr.to_vec())
            .unwrap_or_default();

        for msg in &messages {
            let ts = msg.get("ts").and_then(|v| v.as_str()).unwrap_or("");
            if ts.is_empty() || ts == thread_ts {
                // Skip the parent message (already stored) and empty ts.
                continue;
            }

            let user_id = msg.get("user").and_then(|v| v.as_str());
            let msg_thread_ts = msg.get("thread_ts").and_then(|v| v.as_str());
            let text = msg.get("text").and_then(|v| v.as_str());
            let subtype = msg.get("subtype").and_then(|v| v.as_str());
            let reply_count = msg
                .get("reply_count")
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as i32;

            let raw_json = if config.store_raw_json {
                Some(serde_json::to_string(msg).unwrap_or_default())
            } else {
                None
            };

            store.upsert_message(
                channel_id,
                ts,
                user_id,
                msg_thread_ts,
                text,
                None,
                subtype,
                reply_count,
                raw_json.as_deref(),
            )?;

            // Upsert reactions on thread replies too.
            if let Some(reactions) = msg.get("reactions").and_then(|v| v.as_array()) {
                for reaction in reactions {
                    let emoji = match reaction.get("name").and_then(|v| v.as_str()) {
                        Some(e) => e,
                        None => continue,
                    };
                    if let Some(users) = reaction.get("users").and_then(|v| v.as_array()) {
                        for user_val in users {
                            if let Some(uid) = user_val.as_str() {
                                store.upsert_reaction(channel_id, ts, emoji, uid)?;
                            }
                        }
                    }
                }
            }
        }

        cursor = response
            .get("response_metadata")
            .and_then(|m| m.get("next_cursor"))
            .and_then(|c| c.as_str())
            .filter(|c| !c.is_empty())
            .map(|c| c.to_string());

        if cursor.is_none() {
            break;
        }

        // Rate limit between thread pages.
        tokio::time::sleep(Duration::from_millis(1200)).await;
    }

    Ok(())
}

fn new_spinner(msg: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    if crate::output::is_no_progress() {
        pb.set_draw_target(indicatif::ProgressDrawTarget::hidden());
    }
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.cyan} {msg}")
            .unwrap(),
    );
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(Duration::from_millis(120));
    pb
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backfill_stats_serialize() {
        let stats = BackfillStats {
            channel_id: "C001".to_string(),
            messages_added: 42,
            messages_updated: 3,
            pages_fetched: 5,
            duration_ms: 12345,
        };
        let json = serde_json::to_value(&stats).unwrap();
        assert_eq!(json["channel_id"], "C001");
        assert_eq!(json["messages_added"], 42);
        assert_eq!(json["messages_updated"], 3);
        assert_eq!(json["pages_fetched"], 5);
        assert_eq!(json["duration_ms"], 12345);
    }

    #[test]
    fn test_backfill_stats_default_values() {
        let stats = BackfillStats {
            channel_id: String::new(),
            messages_added: 0,
            messages_updated: 0,
            pages_fetched: 0,
            duration_ms: 0,
        };
        assert_eq!(stats.messages_added, 0);
        assert_eq!(stats.pages_fetched, 0);
    }
}
