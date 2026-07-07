use crate::error::{Result, SlackersError};
use crate::slack::SlackClient;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlashCommandResult {
    pub command: String,
    pub channel: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<String>,
}

pub async fn execute_slash_command(
    client: &SlackClient,
    channel_id: &str,
    command: &str,
    text: &str,
) -> Result<SlashCommandResult> {
    let params = vec![
        ("command".to_string(), command.to_string()),
        ("text".to_string(), text.to_string()),
        ("channel".to_string(), channel_id.to_string()),
        ("blocks".to_string(), "[]".to_string()),
        ("unfurl".to_string(), "[]".to_string()),
        ("unfurl_links".to_string(), "true".to_string()),
    ];

    let resp = client.api_call("chat.command", params).await?;

    let response_text = resp
        .get("response")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    Ok(SlashCommandResult {
        command: command.to_string(),
        channel: channel_id.to_string(),
        response: response_text,
    })
}

pub fn parse_slash_command(input: &str) -> Result<(String, String)> {
    let trimmed = input.trim();
    if !trimmed.starts_with('/') {
        return Err(SlackersError::Other(format!(
            "Not a slash command (must start with /): {}",
            trimmed
        )));
    }

    let parts: Vec<&str> = trimmed.splitn(2, ' ').collect();
    let command = parts[0].to_string();
    let text = parts.get(1).unwrap_or(&"").trim().to_string();

    Ok((command, text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_slash_command_with_text() {
        let (cmd, text) = parse_slash_command("/remind me to check in 1 hour").unwrap();
        assert_eq!(cmd, "/remind");
        assert_eq!(text, "me to check in 1 hour");
    }

    #[test]
    fn test_parse_slash_command_no_text() {
        let (cmd, text) = parse_slash_command("/status").unwrap();
        assert_eq!(cmd, "/status");
        assert_eq!(text, "");
    }

    #[test]
    fn test_parse_slash_command_with_whitespace() {
        let (cmd, text) = parse_slash_command("  /giphy cats  ").unwrap();
        assert_eq!(cmd, "/giphy");
        assert_eq!(text, "cats");
    }

    #[test]
    fn test_parse_slash_command_not_a_command() {
        let result = parse_slash_command("hello world");
        assert!(result.is_err());
    }

    #[test]
    fn test_slash_command_result_serialization() {
        let result = SlashCommandResult {
            command: "/remind".to_string(),
            channel: "C123".to_string(),
            response: Some("Reminder set!".to_string()),
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["command"], "/remind");
        assert_eq!(json["channel"], "C123");
        assert_eq!(json["response"], "Reminder set!");
    }

    #[test]
    fn test_slash_command_result_no_response() {
        let result = SlashCommandResult {
            command: "/status".to_string(),
            channel: "C456".to_string(),
            response: None,
        };
        let json = serde_json::to_value(&result).unwrap();
        assert!(json.get("response").is_none());
    }
}
