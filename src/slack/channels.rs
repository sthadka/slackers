use crate::error::{ParseError, Result, SlackersError};
use crate::slack::SlackClient;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Compact representation of a Slack channel/conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactChannel {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_channel: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_private: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_im: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_mpim: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_member: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_archived: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub purpose: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_members: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<u64>,
}

/// List conversations/channels in the workspace
///
/// Returns a vector of compact channel representations.
/// Supports filtering by types (public_channel, private_channel, mpim, im),
/// excluding archived channels, and pagination.
pub async fn list_conversations(
    client: &SlackClient,
    types: Option<Vec<String>>,
    exclude_archived: bool,
    limit: Option<usize>,
) -> Result<Vec<CompactChannel>> {
    let mut all_channels = Vec::new();
    let mut cursor: Option<String> = None;
    let effective_limit = limit.unwrap_or(usize::MAX);

    // Build types parameter - default to public and private channels
    let types_param = types
        .unwrap_or_else(|| vec!["public_channel".to_string(), "private_channel".to_string()])
        .join(",");

    loop {
        let mut params = vec![
            ("types".to_string(), types_param.clone()),
            ("limit".to_string(), "200".to_string()),
            ("exclude_archived".to_string(), exclude_archived.to_string()),
        ];

        if let Some(c) = cursor {
            params.push(("cursor".to_string(), c));
        }

        let response = client.api_call("conversations.list", params).await?;

        if let Some(channels) = response.get("channels").and_then(|v| v.as_array()) {
            for channel in channels {
                let compact = to_compact_channel(channel);
                all_channels.push(compact);

                if all_channels.len() >= effective_limit {
                    return Ok(all_channels);
                }
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

    Ok(all_channels)
}

/// Get detailed information about a specific conversation/channel
///
/// Returns channel info including topic, purpose, member count if requested.
pub async fn get_conversation_info(
    client: &SlackClient,
    channel_id: &str,
    include_num_members: bool,
) -> Result<CompactChannel> {
    let mut params = vec![("channel".to_string(), channel_id.to_string())];

    if include_num_members {
        params.push(("include_num_members".to_string(), "true".to_string()));
    }

    let response = client.api_call("conversations.info", params).await?;

    let channel = response
        .get("channel")
        .ok_or_else(|| SlackersError::Other("No channel in response".to_string()))?;

    Ok(to_compact_channel(channel))
}

/// Join a conversation/channel
///
/// Returns the joined channel info.
pub async fn join_conversation(client: &SlackClient, channel: &str) -> Result<CompactChannel> {
    let params = vec![("channel".to_string(), channel.to_string())];

    let response = client.api_call("conversations.join", params).await?;

    let channel = response
        .get("channel")
        .ok_or_else(|| SlackersError::Other("No channel in response".to_string()))?;

    Ok(to_compact_channel(channel))
}

/// Leave a conversation/channel
pub async fn leave_conversation(client: &SlackClient, channel_id: &str) -> Result<()> {
    let params = vec![("channel".to_string(), channel_id.to_string())];

    client.api_call("conversations.leave", params).await?;

    Ok(())
}

/// Convert a full Slack channel object to compact representation
fn to_compact_channel(channel: &Value) -> CompactChannel {
    let id = channel
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let name = channel
        .get("name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let is_channel = channel.get("is_channel").and_then(|v| v.as_bool());
    let is_private = channel.get("is_private").and_then(|v| v.as_bool());
    let is_im = channel.get("is_im").and_then(|v| v.as_bool());
    let is_mpim = channel.get("is_mpim").and_then(|v| v.as_bool());
    let is_member = channel.get("is_member").and_then(|v| v.as_bool());
    let is_archived = channel.get("is_archived").and_then(|v| v.as_bool());

    let topic = channel
        .get("topic")
        .and_then(|t| t.get("value"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let purpose = channel
        .get("purpose")
        .and_then(|p| p.get("value"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let num_members = channel
        .get("num_members")
        .and_then(|v| v.as_u64())
        .map(|n| n as u32);

    let created = channel.get("created").and_then(|v| v.as_u64());

    CompactChannel {
        id,
        name,
        is_channel,
        is_private,
        is_im,
        is_mpim,
        is_member,
        is_archived,
        topic,
        purpose,
        num_members,
        created,
    }
}

/// Check if input is a channel ID (matches ^[CDG][A-Z0-9]{8,}$)
#[allow(dead_code)]
pub fn is_channel_id(input: &str) -> bool {
    let re = Regex::new(r"^[CDG][A-Z0-9]{8,}$").unwrap();
    re.is_match(input)
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum ChannelInput {
    Id(String),
    Name(String),
}

/// Normalize channel input to either ID or Name
#[allow(dead_code)]
pub fn normalize_channel_input(input: &str) -> ChannelInput {
    if is_channel_id(input) {
        ChannelInput::Id(input.to_string())
    } else {
        // Strip leading # if present
        let name = input.strip_prefix('#').unwrap_or(input);
        ChannelInput::Name(name.to_string())
    }
}

/// Resolve a channel input (name or ID) to a channel ID
///
/// If already an ID, returns it directly.
/// Otherwise, paginates through conversations.list to find matching name.
#[allow(dead_code)]
pub async fn resolve_channel_id(client: &SlackClient, input: &str) -> Result<String> {
    let normalized = normalize_channel_input(input);

    match normalized {
        ChannelInput::Id(id) => Ok(id),
        ChannelInput::Name(name) => find_channel_by_name(client, &name).await,
    }
}

/// Find a channel ID by name via conversations.list pagination
#[allow(dead_code)]
async fn find_channel_by_name(client: &SlackClient, name: &str) -> Result<String> {
    let mut cursor: Option<String> = None;

    loop {
        let mut params = vec![
            ("types".to_string(), "public_channel,private_channel".to_string()),
            ("limit".to_string(), "200".to_string()),
        ];

        if let Some(c) = &cursor {
            params.push(("cursor".to_string(), c.clone()));
        }

        let response = client.api_call("conversations.list", params).await?;

        // Check channels array
        if let Some(channels) = response.get("channels").and_then(|v| v.as_array()) {
            for channel in channels {
                if let Some(channel_name) = channel.get("name").and_then(|v| v.as_str()) {
                    if channel_name == name {
                        if let Some(id) = channel.get("id").and_then(|v| v.as_str()) {
                            return Ok(id.to_string());
                        }
                    }
                }
            }
        }

        // Check for next cursor
        cursor = response
            .get("response_metadata")
            .and_then(|m| m.get("next_cursor"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty());

        if cursor.is_none() {
            break;
        }
    }

    Err(ParseError::InvalidChannel(format!("Channel not found: #{}", name)).into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_is_channel_id() {
        assert!(is_channel_id("C0123456789"));
        assert!(is_channel_id("D0123456789"));
        assert!(is_channel_id("G0123456789"));
        assert!(is_channel_id("C0123456789ABCDEF"));

        assert!(!is_channel_id("general"));
        assert!(!is_channel_id("#general"));
        assert!(!is_channel_id("X0123456789"));
        assert!(!is_channel_id("C012")); // too short
    }

    #[test]
    fn test_normalize_channel_input() {
        match normalize_channel_input("C0123456789") {
            ChannelInput::Id(id) => assert_eq!(id, "C0123456789"),
            _ => panic!("Expected Id"),
        }

        match normalize_channel_input("#general") {
            ChannelInput::Name(name) => assert_eq!(name, "general"),
            _ => panic!("Expected Name"),
        }

        match normalize_channel_input("general") {
            ChannelInput::Name(name) => assert_eq!(name, "general"),
            _ => panic!("Expected Name"),
        }
    }

    #[test]
    fn test_to_compact_channel_full() {
        let channel = json!({
            "id": "C0123456789",
            "name": "general",
            "is_channel": true,
            "is_private": false,
            "is_im": false,
            "is_mpim": false,
            "is_member": true,
            "is_archived": false,
            "topic": {
                "value": "Company-wide announcements",
                "creator": "U123",
                "last_set": 1234567890
            },
            "purpose": {
                "value": "This channel is for team-wide communication",
                "creator": "U123",
                "last_set": 1234567890
            },
            "num_members": 42,
            "created": 1609459200
        });

        let compact = to_compact_channel(&channel);

        assert_eq!(compact.id, "C0123456789");
        assert_eq!(compact.name, Some("general".to_string()));
        assert_eq!(compact.is_channel, Some(true));
        assert_eq!(compact.is_private, Some(false));
        assert_eq!(compact.is_im, Some(false));
        assert_eq!(compact.is_mpim, Some(false));
        assert_eq!(compact.is_member, Some(true));
        assert_eq!(compact.is_archived, Some(false));
        assert_eq!(compact.topic, Some("Company-wide announcements".to_string()));
        assert_eq!(
            compact.purpose,
            Some("This channel is for team-wide communication".to_string())
        );
        assert_eq!(compact.num_members, Some(42));
        assert_eq!(compact.created, Some(1609459200));
    }

    #[test]
    fn test_to_compact_channel_minimal() {
        let channel = json!({
            "id": "D9876543210",
            "is_im": true
        });

        let compact = to_compact_channel(&channel);

        assert_eq!(compact.id, "D9876543210");
        assert_eq!(compact.is_im, Some(true));
        assert_eq!(compact.name, None);
        assert_eq!(compact.topic, None);
        assert_eq!(compact.purpose, None);
        assert_eq!(compact.num_members, None);
    }

    #[test]
    fn test_to_compact_channel_empty_strings() {
        let channel = json!({
            "id": "C1111111111",
            "name": "test",
            "topic": {
                "value": ""
            },
            "purpose": {
                "value": ""
            }
        });

        let compact = to_compact_channel(&channel);

        assert_eq!(compact.id, "C1111111111");
        assert_eq!(compact.name, Some("test".to_string()));
        // Empty strings should be filtered out
        assert_eq!(compact.topic, None);
        assert_eq!(compact.purpose, None);
    }

    #[test]
    fn test_to_compact_channel_private_channel() {
        let channel = json!({
            "id": "G0123456789",
            "name": "secret-project",
            "is_channel": true,
            "is_private": true,
            "is_member": true,
            "is_archived": false
        });

        let compact = to_compact_channel(&channel);

        assert_eq!(compact.id, "G0123456789");
        assert_eq!(compact.name, Some("secret-project".to_string()));
        assert_eq!(compact.is_channel, Some(true));
        assert_eq!(compact.is_private, Some(true));
        assert_eq!(compact.is_member, Some(true));
    }

    #[test]
    fn test_to_compact_channel_archived() {
        let channel = json!({
            "id": "C2222222222",
            "name": "old-project",
            "is_channel": true,
            "is_archived": true
        });

        let compact = to_compact_channel(&channel);

        assert_eq!(compact.id, "C2222222222");
        assert_eq!(compact.is_archived, Some(true));
    }
}
