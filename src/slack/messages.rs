use crate::error::{Result, SlackersError};
use crate::render::render_message_content;
use crate::slack::SlackClient;
use crate::target::SlackMessageRef;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Compact representation of a Slack message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactSlackMessage {
    pub ts: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_ts: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reactions: Option<Vec<Reaction>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<FileInfo>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reaction {
    pub name: String,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mimetype: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url_private: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url_private_download: Option<String>,
}

pub struct CompactMessageOptions {
    pub max_content_chars: Option<usize>,
    pub include_thread_ts: bool,
}

impl Default for CompactMessageOptions {
    fn default() -> Self {
        Self {
            max_content_chars: Some(4000),
            include_thread_ts: true,
        }
    }
}

/// Fetch a specific message by reference
///
/// Try multiple strategies:
/// 1. conversations.history with exact timestamp
/// 2. Scan thread with conversations.replies if it's a thread reply
/// 3. Try as thread root
pub async fn fetch_message(
    client: &SlackClient,
    msg_ref: &SlackMessageRef,
) -> Result<Value> {
    // Strategy 1: Try conversations.history with exact ts
    let params = vec![
        ("channel".to_string(), msg_ref.channel_id.clone()),
        ("latest".to_string(), msg_ref.message_ts.clone()),
        ("inclusive".to_string(), "true".to_string()),
        ("limit".to_string(), "5".to_string()),
    ];

    let response = client.api_call("conversations.history", params).await?;

    if let Some(messages) = response.get("messages").and_then(|v| v.as_array()) {
        // Find exact match
        for msg in messages {
            if let Some(ts) = msg.get("ts").and_then(|v| v.as_str()) {
                if ts == msg_ref.message_ts {
                    return Ok(msg.clone());
                }
            }
        }
    }

    // Strategy 2: If we have a thread_ts hint, try scanning the thread
    if let Some(thread_ts) = &msg_ref.thread_ts_hint {
        if let Ok(thread) = fetch_thread(client, &msg_ref.channel_id, thread_ts).await {
            for msg in thread {
                if let Some(ts) = msg.get("ts").and_then(|v| v.as_str()) {
                    if ts == msg_ref.message_ts {
                        return Ok(msg);
                    }
                }
            }
        }
    }

    // Strategy 3: Try as thread root
    if let Ok(thread) = fetch_thread(client, &msg_ref.channel_id, &msg_ref.message_ts).await {
        if let Some(root) = thread.first() {
            return Ok(root.clone());
        }
    }

    Err(SlackersError::Other(format!(
        "Message not found: {} in channel {}",
        msg_ref.message_ts, msg_ref.channel_id
    )))
}

/// Fetch all messages in a thread
///
/// Returns messages in chronological order
pub async fn fetch_thread(
    client: &SlackClient,
    channel_id: &str,
    thread_ts: &str,
) -> Result<Vec<Value>> {
    let mut all_messages = Vec::new();
    let mut cursor: Option<String> = None;

    loop {
        let mut params = vec![
            ("channel".to_string(), channel_id.to_string()),
            ("ts".to_string(), thread_ts.to_string()),
            ("limit".to_string(), "200".to_string()),
        ];

        if let Some(c) = cursor {
            params.push(("cursor".to_string(), c));
        }

        let response = client.api_call("conversations.replies", params).await?;

        if let Some(messages) = response.get("messages").and_then(|v| v.as_array()) {
            all_messages.extend(messages.iter().cloned());
        }

        // Check for pagination
        cursor = response
            .get("response_metadata")
            .and_then(|m| m.get("next_cursor"))
            .and_then(|c| c.as_str())
            .filter(|c| !c.is_empty())
            .map(|c| c.to_string());

        if cursor.is_none() {
            break;
        }
    }

    // Sort chronologically by ts
    all_messages.sort_by(|a, b| {
        let ts_a = a.get("ts").and_then(|v| v.as_str()).unwrap_or("");
        let ts_b = b.get("ts").and_then(|v| v.as_str()).unwrap_or("");
        ts_a.cmp(ts_b)
    });

    Ok(all_messages)
}

/// Options for listing channel messages
#[allow(dead_code)]
pub struct ListMessagesOptions {
    /// Maximum number of messages to return (default: 100)
    pub limit: Option<usize>,
    /// Only messages after this timestamp (exclusive unless inclusive is true)
    pub oldest: Option<String>,
    /// Only messages before this timestamp (exclusive unless inclusive is true)
    pub latest: Option<String>,
    /// Include messages with timestamps matching oldest or latest
    pub inclusive: bool,
}

impl Default for ListMessagesOptions {
    fn default() -> Self {
        Self {
            limit: Some(100),
            oldest: None,
            latest: None,
            inclusive: false,
        }
    }
}

/// List messages from a channel with pagination and time-range filtering
///
/// Returns messages in reverse chronological order (newest first, as returned by Slack API)
#[allow(dead_code)]
pub async fn list_channel_messages(
    client: &SlackClient,
    channel_id: &str,
    options: ListMessagesOptions,
) -> Result<Vec<Value>> {
    let mut all_messages = Vec::new();
    let mut cursor: Option<String> = None;
    let effective_limit = options.limit.unwrap_or(usize::MAX);

    loop {
        let mut params = vec![
            ("channel".to_string(), channel_id.to_string()),
            ("limit".to_string(), "200".to_string()),
        ];

        if let Some(ref oldest) = options.oldest {
            params.push(("oldest".to_string(), oldest.clone()));
        }

        if let Some(ref latest) = options.latest {
            params.push(("latest".to_string(), latest.clone()));
        }

        if options.inclusive {
            params.push(("inclusive".to_string(), "true".to_string()));
        }

        if let Some(c) = cursor {
            params.push(("cursor".to_string(), c));
        }

        let response = client.api_call("conversations.history", params).await?;

        if let Some(messages) = response.get("messages").and_then(|v| v.as_array()) {
            all_messages.extend(messages.iter().cloned());

            if all_messages.len() >= effective_limit {
                all_messages.truncate(effective_limit);
                break;
            }
        }

        // Check for pagination
        cursor = response
            .get("response_metadata")
            .and_then(|m| m.get("next_cursor"))
            .and_then(|c| c.as_str())
            .filter(|c| !c.is_empty())
            .map(|c| c.to_string());

        if cursor.is_none() {
            break;
        }
    }

    Ok(all_messages)
}

/// Message filter criteria
pub struct MessageFilter {
    /// Filter by user ID or name
    pub user: Option<String>,
    /// Only messages with links
    pub has_link: bool,
    /// Only messages with file attachments
    pub has_file: bool,
    /// Only messages with reactions
    pub has_reaction: bool,
}

impl Default for MessageFilter {
    fn default() -> Self {
        Self {
            user: None,
            has_link: false,
            has_file: false,
            has_reaction: false,
        }
    }
}

/// Filter messages based on criteria
pub fn filter_messages(messages: Vec<Value>, filter: &MessageFilter) -> Vec<Value> {
    messages
        .into_iter()
        .filter(|msg| {
            // Filter by user
            if let Some(ref user_filter) = filter.user {
                if let Some(user) = msg.get("user").and_then(|v| v.as_str()) {
                    if user != user_filter {
                        return false;
                    }
                } else {
                    return false;
                }
            }

            // Filter by has_link
            if filter.has_link {
                let has_link = msg
                    .get("text")
                    .and_then(|v| v.as_str())
                    .map(|text| text.contains("http://") || text.contains("https://"))
                    .unwrap_or(false)
                    || msg
                        .get("attachments")
                        .and_then(|v| v.as_array())
                        .map(|arr| !arr.is_empty())
                        .unwrap_or(false);

                if !has_link {
                    return false;
                }
            }

            // Filter by has_file
            if filter.has_file {
                let has_file = msg
                    .get("files")
                    .and_then(|v| v.as_array())
                    .map(|arr| !arr.is_empty())
                    .unwrap_or(false);

                if !has_file {
                    return false;
                }
            }

            // Filter by has_reaction
            if filter.has_reaction {
                let has_reaction = msg
                    .get("reactions")
                    .and_then(|v| v.as_array())
                    .map(|arr| !arr.is_empty())
                    .unwrap_or(false);

                if !has_reaction {
                    return false;
                }
            }

            true
        })
        .collect()
}

/// Convert a full Slack message to compact format
pub fn to_compact_message(msg: &Value, options: &CompactMessageOptions) -> CompactSlackMessage {
    let ts = msg
        .get("ts")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let user = msg.get("user").and_then(|v| v.as_str()).map(|s| s.to_string());

    // Render message content to Markdown
    let mut text = render_message_content(msg);

    // Truncate if needed
    if let Some(max_chars) = options.max_content_chars {
        if text.len() > max_chars {
            text.truncate(max_chars);
            text.push_str("...");
        }
    }

    let thread_ts = if options.include_thread_ts {
        msg.get("thread_ts")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    } else {
        None
    };

    let reply_count = msg
        .get("reply_count")
        .and_then(|v| v.as_u64())
        .map(|n| n as u32);

    // Extract reactions
    let reactions = msg
        .get("reactions")
        .and_then(|v| v.as_array())
        .map(|reactions_array| {
            reactions_array
                .iter()
                .filter_map(|r| {
                    let name = r.get("name")?.as_str()?.to_string();
                    let count = r.get("count")?.as_u64()? as u32;
                    Some(Reaction { name, count })
                })
                .collect::<Vec<_>>()
        })
        .filter(|r| !r.is_empty());

    // Extract files
    let files = msg
        .get("files")
        .and_then(|v| v.as_array())
        .map(|files_array| {
            files_array
                .iter()
                .filter_map(|f| {
                    let id = f.get("id")?.as_str()?.to_string();
                    let name = f.get("name")?.as_str()?.to_string();
                    let mimetype = f.get("mimetype").and_then(|v| v.as_str()).map(|s| s.to_string());
                    let url_private = f
                        .get("url_private")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    let url_private_download = f
                        .get("url_private_download")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());

                    Some(FileInfo {
                        id,
                        name,
                        mimetype,
                        url_private,
                        url_private_download,
                    })
                })
                .collect::<Vec<_>>()
        })
        .filter(|f| !f.is_empty());

    CompactSlackMessage {
        ts,
        user,
        text,
        thread_ts,
        reply_count,
        reactions,
        files,
    }
}

/// Get summary of a thread (root message + reply count)
pub async fn get_thread_summary(
    client: &SlackClient,
    channel_id: &str,
    thread_ts: &str,
) -> Result<(Value, usize)> {
    let thread = fetch_thread(client, channel_id, thread_ts).await?;
    let reply_count = thread.len().saturating_sub(1); // Exclude root message

    let root = thread
        .first()
        .cloned()
        .ok_or_else(|| SlackersError::Other("Thread is empty".to_string()))?;

    Ok((root, reply_count))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_to_compact_message_simple() {
        let msg = json!({
            "ts": "1234567890.123456",
            "user": "U0123456789",
            "text": "Hello, world!"
        });

        let compact = to_compact_message(&msg, &CompactMessageOptions::default());

        assert_eq!(compact.ts, "1234567890.123456");
        assert_eq!(compact.user, Some("U0123456789".to_string()));
        assert_eq!(compact.text, "Hello, world!");
        assert_eq!(compact.thread_ts, None);
        assert_eq!(compact.reply_count, None);
    }

    #[test]
    fn test_to_compact_message_with_thread() {
        let msg = json!({
            "ts": "1234567890.123456",
            "user": "U0123456789",
            "text": "Thread root",
            "thread_ts": "1234567890.123456",
            "reply_count": 5
        });

        let compact = to_compact_message(&msg, &CompactMessageOptions::default());

        assert_eq!(compact.thread_ts, Some("1234567890.123456".to_string()));
        assert_eq!(compact.reply_count, Some(5));
    }

    #[test]
    fn test_to_compact_message_with_reactions() {
        let msg = json!({
            "ts": "1234567890.123456",
            "user": "U0123456789",
            "text": "Great!",
            "reactions": [
                {"name": "thumbsup", "count": 3},
                {"name": "rocket", "count": 1}
            ]
        });

        let compact = to_compact_message(&msg, &CompactMessageOptions::default());

        assert!(compact.reactions.is_some());
        let reactions = compact.reactions.unwrap();
        assert_eq!(reactions.len(), 2);
        assert_eq!(reactions[0].name, "thumbsup");
        assert_eq!(reactions[0].count, 3);
    }

    #[test]
    fn test_to_compact_message_with_files() {
        let msg = json!({
            "ts": "1234567890.123456",
            "user": "U0123456789",
            "text": "Check this out",
            "files": [
                {
                    "id": "F0123456789",
                    "name": "document.pdf",
                    "mimetype": "application/pdf",
                    "url_private": "https://files.slack.com/files-pri/T.../document.pdf"
                }
            ]
        });

        let compact = to_compact_message(&msg, &CompactMessageOptions::default());

        assert!(compact.files.is_some());
        let files = compact.files.unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].id, "F0123456789");
        assert_eq!(files[0].name, "document.pdf");
    }

    #[test]
    fn test_to_compact_message_truncation() {
        let msg = json!({
            "ts": "1234567890.123456",
            "user": "U0123456789",
            "text": "a".repeat(5000)
        });

        let options = CompactMessageOptions {
            max_content_chars: Some(100),
            ..Default::default()
        };

        let compact = to_compact_message(&msg, &options);

        assert!(compact.text.len() <= 103); // 100 + "..."
        assert!(compact.text.ends_with("..."));
    }

    #[test]
    fn test_to_compact_message_no_thread_ts() {
        let msg = json!({
            "ts": "1234567890.123456",
            "user": "U0123456789",
            "text": "Hello",
            "thread_ts": "1234567890.123456"
        });

        let options = CompactMessageOptions {
            include_thread_ts: false,
            ..Default::default()
        };

        let compact = to_compact_message(&msg, &options);

        assert_eq!(compact.thread_ts, None);
    }

    #[test]
    fn test_list_messages_options_default() {
        let options = ListMessagesOptions::default();

        assert_eq!(options.limit, Some(100));
        assert_eq!(options.oldest, None);
        assert_eq!(options.latest, None);
        assert_eq!(options.inclusive, false);
    }

    #[test]
    fn test_list_messages_options_with_time_range() {
        let options = ListMessagesOptions {
            limit: Some(50),
            oldest: Some("1609459200.000000".to_string()),
            latest: Some("1609545600.000000".to_string()),
            inclusive: true,
        };

        assert_eq!(options.limit, Some(50));
        assert_eq!(options.oldest, Some("1609459200.000000".to_string()));
        assert_eq!(options.latest, Some("1609545600.000000".to_string()));
        assert_eq!(options.inclusive, true);
    }

    #[test]
    fn test_filter_messages_by_user() {
        let messages = vec![
            json!({"ts": "1.0", "user": "U123", "text": "Hello"}),
            json!({"ts": "2.0", "user": "U456", "text": "Hi"}),
            json!({"ts": "3.0", "user": "U123", "text": "Bye"}),
        ];

        let filter = MessageFilter {
            user: Some("U123".to_string()),
            ..Default::default()
        };

        let filtered = filter_messages(messages, &filter);
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0]["ts"], "1.0");
        assert_eq!(filtered[1]["ts"], "3.0");
    }

    #[test]
    fn test_filter_messages_has_link() {
        let messages = vec![
            json!({"ts": "1.0", "user": "U123", "text": "Check this https://example.com"}),
            json!({"ts": "2.0", "user": "U456", "text": "No link here"}),
            json!({"ts": "3.0", "user": "U123", "text": "Visit http://test.com"}),
        ];

        let filter = MessageFilter {
            has_link: true,
            ..Default::default()
        };

        let filtered = filter_messages(messages, &filter);
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0]["ts"], "1.0");
        assert_eq!(filtered[1]["ts"], "3.0");
    }

    #[test]
    fn test_filter_messages_has_file() {
        let messages = vec![
            json!({"ts": "1.0", "user": "U123", "text": "Doc", "files": [{"id": "F1"}]}),
            json!({"ts": "2.0", "user": "U456", "text": "No files"}),
            json!({"ts": "3.0", "user": "U123", "text": "Image", "files": [{"id": "F2"}]}),
        ];

        let filter = MessageFilter {
            has_file: true,
            ..Default::default()
        };

        let filtered = filter_messages(messages, &filter);
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0]["ts"], "1.0");
        assert_eq!(filtered[1]["ts"], "3.0");
    }

    #[test]
    fn test_filter_messages_has_reaction() {
        let messages = vec![
            json!({"ts": "1.0", "user": "U123", "text": "Great!", "reactions": [{"name": "thumbsup", "count": 1}]}),
            json!({"ts": "2.0", "user": "U456", "text": "Meh"}),
            json!({"ts": "3.0", "user": "U123", "text": "Nice", "reactions": [{"name": "rocket", "count": 2}]}),
        ];

        let filter = MessageFilter {
            has_reaction: true,
            ..Default::default()
        };

        let filtered = filter_messages(messages, &filter);
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0]["ts"], "1.0");
        assert_eq!(filtered[1]["ts"], "3.0");
    }

    #[test]
    fn test_filter_messages_combined() {
        let messages = vec![
            json!({"ts": "1.0", "user": "U123", "text": "https://example.com", "files": [{"id": "F1"}]}),
            json!({"ts": "2.0", "user": "U123", "text": "Just text"}),
            json!({"ts": "3.0", "user": "U456", "text": "https://test.com", "files": [{"id": "F2"}]}),
        ];

        let filter = MessageFilter {
            user: Some("U123".to_string()),
            has_link: true,
            has_file: true,
            ..Default::default()
        };

        let filtered = filter_messages(messages, &filter);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0]["ts"], "1.0");
    }
}
