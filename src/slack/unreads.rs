use crate::error::Result;
use crate::render::render_message_content;
use crate::slack::SlackClient;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnreadChannel {
    pub channel_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel_name: Option<String>,
    pub channel_type: String,
    pub unread_count: u64,
    pub mention_count: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub messages: Option<Vec<UnreadMessage>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnreadMessage {
    pub ts: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bot_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_ts: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_count: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadUnreads {
    pub has_unreads: bool,
    pub mention_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnreadsResult {
    pub channels: Vec<UnreadChannel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threads: Option<ThreadUnreads>,
}

pub struct FetchUnreadsOptions {
    pub include_messages: bool,
    pub max_messages_per_channel: usize,
    pub max_body_chars: i64,
    pub skip_system_messages: bool,
}

impl Default for FetchUnreadsOptions {
    fn default() -> Self {
        Self {
            include_messages: true,
            max_messages_per_channel: 10,
            max_body_chars: 4000,
            skip_system_messages: true,
        }
    }
}

const SYSTEM_SUBTYPES: &[&str] = &[
    "channel_join",
    "channel_leave",
    "channel_topic",
    "channel_purpose",
    "channel_name",
    "channel_archive",
    "channel_unarchive",
    "group_join",
    "group_leave",
    "group_topic",
    "group_purpose",
    "group_name",
    "group_archive",
    "group_unarchive",
];

struct CountsEntry {
    id: String,
    entry_type: String,
    has_unreads: bool,
    unread_count: Option<u64>,
    unread_count_display: Option<u64>,
    mention_count: u64,
    last_read: Option<String>,
}

pub async fn fetch_unreads(
    client: &SlackClient,
    options: &FetchUnreadsOptions,
) -> Result<UnreadsResult> {
    let resp = client
        .api_call(
            "client.counts",
            vec![("thread_count_by_channel".to_string(), "true".to_string())],
        )
        .await?;

    let mut all_entries = Vec::new();

    for (key, entry_type) in &[
        ("channels", "channel"),
        ("mpims", "mpim"),
        ("ims", "dm"),
    ] {
        if let Some(arr) = resp.get(*key).and_then(|v| v.as_array()) {
            for item in arr {
                let has_unreads = item
                    .get("has_unreads")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if !has_unreads {
                    continue;
                }
                all_entries.push(CountsEntry {
                    id: item.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    entry_type: entry_type.to_string(),
                    has_unreads,
                    unread_count: item.get("unread_count").and_then(|v| v.as_u64()),
                    unread_count_display: item.get("unread_count_display").and_then(|v| v.as_u64()),
                    mention_count: item.get("mention_count").and_then(|v| v.as_u64()).unwrap_or(0),
                    last_read: item
                        .get("last_read")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                });
            }
        }
    }

    let mut channels = Vec::new();

    for entry in &all_entries {
        let (name, resolved_type) = resolve_channel_info(client, &entry.id, &entry.entry_type).await;

        let unread_count = entry
            .unread_count_display
            .or(entry.unread_count)
            .unwrap_or(if entry.has_unreads { 1 } else { 0 });

        let mut channel = UnreadChannel {
            channel_id: entry.id.clone(),
            channel_name: name,
            channel_type: resolved_type,
            unread_count,
            mention_count: entry.mention_count,
            messages: None,
        };

        if options.include_messages {
            if let Some(last_read) = &entry.last_read {
                match fetch_channel_messages(
                    client,
                    &entry.id,
                    last_read,
                    options,
                    &entry,
                )
                .await
                {
                    Ok((msgs, updated_count)) => {
                        if let Some(count) = updated_count {
                            channel.unread_count = count;
                        }
                        if !msgs.is_empty() {
                            channel.messages = Some(msgs);
                        }
                    }
                    Err(_) => {}
                }
            }
        }

        channels.push(channel);
    }

    channels.sort_by(|a, b| {
        b.mention_count
            .cmp(&a.mention_count)
            .then(b.unread_count.cmp(&a.unread_count))
    });

    let threads = resp
        .get("threads")
        .and_then(|t| {
            let has_unreads = t.get("has_unreads").and_then(|v| v.as_bool()).unwrap_or(false);
            if has_unreads {
                Some(ThreadUnreads {
                    has_unreads: true,
                    mention_count: t
                        .get("mention_count")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0),
                })
            } else {
                None
            }
        });

    Ok(UnreadsResult { channels, threads })
}

async fn resolve_channel_info(
    client: &SlackClient,
    channel_id: &str,
    default_type: &str,
) -> (Option<String>, String) {
    let params = vec![("channel".to_string(), channel_id.to_string())];
    let info = match client.api_call("conversations.info", params).await {
        Ok(resp) => resp,
        Err(_) => return (None, default_type.to_string()),
    };

    let ch = match info.get("channel") {
        Some(ch) => ch,
        None => return (None, default_type.to_string()),
    };

    let mut name = ch
        .get("name")
        .and_then(|v| v.as_str())
        .or_else(|| ch.get("name_normalized").and_then(|v| v.as_str()))
        .map(|s| s.to_string());

    let is_im = ch.get("is_im").and_then(|v| v.as_bool()).unwrap_or(false);
    let is_mpim = ch.get("is_mpim").and_then(|v| v.as_bool()).unwrap_or(false);

    let resolved_type = if is_im {
        if name.is_none() {
            if let Some(user_id) = ch.get("user").and_then(|v| v.as_str()) {
                name = resolve_user_display_name(client, user_id).await;
            }
        }
        "dm".to_string()
    } else if is_mpim {
        "mpim".to_string()
    } else {
        "channel".to_string()
    };

    (name, resolved_type)
}

async fn resolve_user_display_name(client: &SlackClient, user_id: &str) -> Option<String> {
    let params = vec![("user".to_string(), user_id.to_string())];
    let resp = client.api_call("users.info", params).await.ok()?;
    let user = resp.get("user")?;
    let profile = user.get("profile");

    profile
        .and_then(|p| p.get("display_name"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .or_else(|| user.get("real_name").and_then(|v| v.as_str()))
        .or_else(|| user.get("name").and_then(|v| v.as_str()))
        .map(|s| s.to_string())
}

async fn fetch_channel_messages(
    client: &SlackClient,
    channel_id: &str,
    last_read: &str,
    options: &FetchUnreadsOptions,
    entry: &CountsEntry,
) -> Result<(Vec<UnreadMessage>, Option<u64>)> {
    let params = vec![
        ("channel".to_string(), channel_id.to_string()),
        ("oldest".to_string(), last_read.to_string()),
        (
            "limit".to_string(),
            options.max_messages_per_channel.to_string(),
        ),
        ("inclusive".to_string(), "false".to_string()),
    ];

    let history = client.api_call("conversations.history", params).await?;

    let raw_msgs = history
        .get("messages")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let filtered: Vec<&Value> = if options.skip_system_messages {
        raw_msgs
            .iter()
            .filter(|m| {
                let subtype = m.get("subtype").and_then(|v| v.as_str());
                match subtype {
                    None => true,
                    Some(st) => !SYSTEM_SUBTYPES.contains(&st),
                }
            })
            .collect()
    } else {
        raw_msgs.iter().collect()
    };

    let updated_count = if entry.unread_count_display.is_none() && entry.unread_count.is_none() {
        let count = filtered.len() as u64;
        let has_more = history
            .get("has_more")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        Some(if has_more && count < 2 { 2 } else { count })
    } else {
        None
    };

    let mut messages: Vec<UnreadMessage> = filtered
        .iter()
        .map(|m| {
            let rendered = render_message_content(m);
            let content = if options.max_body_chars >= 0 && rendered.len() > options.max_body_chars as usize {
                let mut truncated = rendered[..options.max_body_chars as usize].to_string();
                truncated.push_str("\n...");
                truncated
            } else {
                rendered
            };

            UnreadMessage {
                ts: m.get("ts").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                user_id: m.get("user").and_then(|v| v.as_str()).map(|s| s.to_string()),
                bot_id: m.get("bot_id").and_then(|v| v.as_str()).map(|s| s.to_string()),
                content: if content.is_empty() { None } else { Some(content) },
                thread_ts: m.get("thread_ts").and_then(|v| v.as_str()).map(|s| s.to_string()),
                reply_count: m.get("reply_count").and_then(|v| v.as_u64()),
            }
        })
        .collect();

    messages.sort_by(|a, b| {
        a.ts.parse::<f64>()
            .unwrap_or(0.0)
            .partial_cmp(&b.ts.parse::<f64>().unwrap_or(0.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok((messages, updated_count))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_system_subtypes_filter() {
        let system_msg = json!({
            "subtype": "channel_join",
            "ts": "1234.5678",
            "text": "joined"
        });

        let normal_msg = json!({
            "ts": "1234.5679",
            "text": "hello"
        });

        let subtype = system_msg.get("subtype").and_then(|v| v.as_str());
        assert!(SYSTEM_SUBTYPES.contains(&subtype.unwrap()));

        let subtype = normal_msg.get("subtype").and_then(|v| v.as_str());
        assert!(subtype.is_none());
    }

    #[test]
    fn test_unreads_result_serialization() {
        let result = UnreadsResult {
            channels: vec![UnreadChannel {
                channel_id: "C123".to_string(),
                channel_name: Some("general".to_string()),
                channel_type: "channel".to_string(),
                unread_count: 5,
                mention_count: 2,
                messages: Some(vec![UnreadMessage {
                    ts: "1234.5678".to_string(),
                    user_id: Some("U123".to_string()),
                    bot_id: None,
                    content: Some("hello".to_string()),
                    thread_ts: None,
                    reply_count: None,
                }]),
            }],
            threads: Some(ThreadUnreads {
                has_unreads: true,
                mention_count: 1,
            }),
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["channels"][0]["channel_id"], "C123");
        assert_eq!(json["channels"][0]["mention_count"], 2);
        assert_eq!(json["threads"]["mention_count"], 1);
    }

    #[test]
    fn test_unread_message_skip_serializing_none() {
        let msg = UnreadMessage {
            ts: "1234.5678".to_string(),
            user_id: Some("U123".to_string()),
            bot_id: None,
            content: Some("test".to_string()),
            thread_ts: None,
            reply_count: None,
        };

        let json = serde_json::to_value(&msg).unwrap();
        assert!(json.get("user_id").is_some());
        assert!(json.get("bot_id").is_none());
        assert!(json.get("thread_ts").is_none());
        assert!(json.get("reply_count").is_none());
    }

    #[test]
    fn test_fetch_unreads_options_default() {
        let opts = FetchUnreadsOptions::default();
        assert!(opts.include_messages);
        assert_eq!(opts.max_messages_per_channel, 10);
        assert_eq!(opts.max_body_chars, 4000);
        assert!(opts.skip_system_messages);
    }
}
