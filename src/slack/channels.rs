use crate::error::{ParseError, Result};
use crate::slack::SlackClient;
use regex::Regex;

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
}
