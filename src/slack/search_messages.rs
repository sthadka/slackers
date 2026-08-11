use crate::auth::WorkspaceAuth;
use crate::error::Result;
use crate::slack::{
    download_file, fetch_message, search_messages_raw_sorted, to_compact_message,
    CompactMessageOptions, CompactSlackMessage, SlackClient, SortOrder,
};
use crate::target::{parse_slack_message_url, SlackMessageRef};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub enum ContentType {
    Any,
    Text,
    Image,
    Snippet,
    File,
}

pub struct SearchMessagesInput<'a> {
    pub auth: &'a WorkspaceAuth,
    pub workspace_url: Option<&'a str>,
    pub query: &'a str,
    pub limit: usize,
    pub max_content_chars: usize,
    pub content_type: ContentType,
    pub download: bool,
    /// Sort order for results (default: Timestamp / newest first)
    pub sort: SortOrder,
}

/// Search for messages using the search.messages API
///
/// Returns compact messages matching the search query.
pub async fn search_messages(
    client: &SlackClient,
    input: SearchMessagesInput<'_>,
) -> Result<Vec<CompactSlackMessage>> {
    // Get raw matches from search API
    let raw_matches = search_messages_raw_sorted(client, input.query, input.limit, &input.sort).await?;

    if raw_matches.is_empty() {
        return Ok(Vec::new());
    }

    // Extract message references from matches
    let mut message_refs = Vec::new();
    for match_item in &raw_matches {
        let ts = match_item
            .get("ts")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string());

        let channel_id = match_item
            .get("channel")
            .and_then(|c| {
                if let Some(obj) = c.as_object() {
                    obj.get("id")
                        .and_then(|v| v.as_str())
                        .or_else(|| obj.get("name").and_then(|v| v.as_str()))
                } else {
                    None
                }
            })
            .map(|s| s.to_string());

        let permalink = match_item
            .get("permalink")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        if let (Some(ts), Some(channel_id)) = (ts, channel_id) {
            message_refs.push((channel_id, ts, permalink));
            if message_refs.len() >= input.limit {
                break;
            }
        }
    }

    // Fetch full messages and optionally download files
    let mut downloaded_paths: HashMap<String, PathBuf> = HashMap::new();
    let mut results = Vec::new();

    for (channel_id, ts, permalink) in message_refs {
        // Try to fetch the full message
        let message = match fetch_message_with_fallback(
            client,
            &channel_id,
            &ts,
            permalink.as_deref(),
            input.workspace_url,
        )
        .await
        {
            Ok(msg) => msg,
            Err(_) => continue,
        };

        // Download files if requested
        if input.download {
            if let Some(files) = message.get("files").and_then(|f| f.as_array()) {
                for file in files {
                    if let Some(file_id) = file.get("id").and_then(|id| id.as_str()) {
                        if !downloaded_paths.contains_key(file_id) {
                            if let Ok(path) =
                                download_file(client, file, input.auth, input.workspace_url).await
                            {
                                downloaded_paths.insert(file_id.to_string(), path);
                            }
                        }
                    }
                }
            }
        }

        // Convert to compact message
        let options = CompactMessageOptions {
            max_content_chars: Some(input.max_content_chars),
            include_thread_ts: false,
        };

        let compact = to_compact_message(&message, &options);

        // Apply content type filter
        if !passes_content_type_filter(&compact, &input.content_type) {
            continue;
        }

        results.push(compact);
        if results.len() >= input.limit {
            break;
        }
    }

    Ok(results)
}

/// Fetch message with fallback strategies
async fn fetch_message_with_fallback(
    client: &SlackClient,
    channel_id: &str,
    ts: &str,
    permalink: Option<&str>,
    workspace_url: Option<&str>,
) -> Result<Value> {
    // Try to parse permalink for additional context
    let parsed = permalink
        .and_then(|p| parse_slack_message_url(p).ok())
        .or_else(|| {
            workspace_url.and_then(|w| {
                parse_slack_message_url(&format!("{}/archives/{}/p{}", w, channel_id, ts.replace('.', "")))
                    .ok()
            })
        });

    // Build message reference
    let msg_ref = if let Some(parsed_ref) = parsed {
        parsed_ref
    } else {
        // Construct a basic reference
        SlackMessageRef {
            workspace_url: workspace_url.unwrap_or("").to_string(),
            channel_id: channel_id.to_string(),
            message_ts: ts.to_string(),
            thread_ts_hint: None,
            raw: format!("{}:{}", channel_id, ts),
            possibly_truncated: false,
        }
    };

    fetch_message(client, &msg_ref).await
}

/// Check if message passes content type filter
fn passes_content_type_filter(msg: &CompactSlackMessage, content_type: &ContentType) -> bool {
    use crate::slack::messages::FileInfo;

    match content_type {
        ContentType::Any => true,
        ContentType::Text => msg.files.is_none() || msg.files.as_ref().unwrap().is_empty(),
        ContentType::File => msg.files.is_some() && !msg.files.as_ref().unwrap().is_empty(),
        ContentType::Snippet => {
            // Note: FileInfo doesn't have a mode field, so snippet filtering is limited
            // This would need to be determined from the raw message data
            false
        }
        ContentType::Image => msg
            .files
            .as_ref()
            .map(|files: &Vec<FileInfo>| {
                files
                    .iter()
                    .any(|f| f.mimetype.as_ref().map(|m| m.starts_with("image/")).unwrap_or(false))
            })
            .unwrap_or(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_type_filter_text() {
        let msg = CompactSlackMessage {
            ts: "123.456".to_string(),
            user: Some("U123".to_string()),
            text: "Hello".to_string(),
            thread_ts: None,
            reply_count: None,
            reactions: None,
            files: None,
        };

        assert!(passes_content_type_filter(&msg, &ContentType::Any));
        assert!(passes_content_type_filter(&msg, &ContentType::Text));
        assert!(!passes_content_type_filter(&msg, &ContentType::File));
    }

    #[test]
    fn test_content_type_filter_with_files() {
        use crate::slack::messages::FileInfo;

        let msg = CompactSlackMessage {
            ts: "123.456".to_string(),
            user: Some("U123".to_string()),
            text: "Hello".to_string(),
            thread_ts: None,
            reply_count: None,
            reactions: None,
            files: Some(vec![FileInfo {
                id: "F123".to_string(),
                name: "test.png".to_string(),
                mimetype: Some("image/png".to_string()),
                url_private: None,
                url_private_download: None,
            }]),
        };

        assert!(passes_content_type_filter(&msg, &ContentType::Any));
        assert!(!passes_content_type_filter(&msg, &ContentType::Text));
        assert!(passes_content_type_filter(&msg, &ContentType::File));
        assert!(passes_content_type_filter(&msg, &ContentType::Image));
    }
}
